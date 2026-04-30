mod edge_reader;
mod metrics;
mod model;
mod partition;
mod switch;
mod trace;

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::{
    edge_reader::{stream_edge_list, Edge},
    metrics::Metrics,
    model::{make_bfs_packet, validate_lanes, EdgeTarget, DEFAULT_GRID, DEFAULT_LANES},
    partition::{owner_count, validate_grid},
    switch::{ReduceSwitch, SwitchConfig},
    trace::{TraceHeader, TraceReader, TraceWriter, UNKNOWN_PACKET_COUNT},
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Smoke {
        #[arg(long, default_value = "data/smoke.edgelist")]
        out: PathBuf,
    },
    Run {
        #[arg(long)]
        edge_list: PathBuf,
        #[arg(long, default_value_t = DEFAULT_GRID)]
        grid: u64,
        #[arg(long, default_value_t = DEFAULT_LANES)]
        lanes: u64,
        #[arg(long, default_value_t = 1)]
        epoch: u16,
        #[arg(long, value_enum, default_value_t = TargetArg::Dst)]
        target: TargetArg,
        #[arg(long, default_value_t = 4)]
        stages: usize,
        #[arg(long, default_value_t = 4096)]
        sets: usize,
        #[arg(long, default_value_t = 4)]
        ways: usize,
        #[arg(long)]
        limit_edges: Option<u64>,
        #[arg(long)]
        progress_every: Option<u64>,
    },
    GenTrace {
        #[arg(long)]
        edge_list: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = DEFAULT_GRID)]
        grid: u64,
        #[arg(long, default_value_t = DEFAULT_LANES)]
        lanes: u64,
        #[arg(long, default_value_t = 1)]
        epoch: u16,
        #[arg(long, value_enum, default_value_t = TargetArg::Dst)]
        target: TargetArg,
        #[arg(long)]
        limit_edges: Option<u64>,
        #[arg(long)]
        progress_every: Option<u64>,
    },
    Simulate {
        #[arg(long)]
        trace: PathBuf,
        #[arg(long, default_value_t = 4)]
        stages: usize,
        #[arg(long, default_value_t = 4096)]
        sets: usize,
        #[arg(long, default_value_t = 4)]
        ways: usize,
        #[arg(long)]
        progress_every: Option<u64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum TargetArg {
    Src,
    Dst,
}

impl From<TargetArg> for EdgeTarget {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Src => Self::Src,
            TargetArg::Dst => Self::Dst,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Smoke { out } => write_smoke_graph(&out),
        Commands::Run {
            edge_list,
            grid,
            lanes,
            epoch,
            target,
            stages,
            sets,
            ways,
            limit_edges,
            progress_every,
        } => run_edge_stream(RunConfig {
            edge_list,
            grid,
            lanes,
            epoch,
            target: target.into(),
            switch: SwitchConfig { stages, sets, ways },
            limit_edges,
            progress_every,
        }),
        Commands::GenTrace {
            edge_list,
            out,
            grid,
            lanes,
            epoch,
            target,
            limit_edges,
            progress_every,
        } => gen_trace(GenTraceConfig {
            edge_list,
            out,
            grid,
            lanes,
            epoch,
            target: target.into(),
            limit_edges,
            progress_every,
        }),
        Commands::Simulate {
            trace,
            stages,
            sets,
            ways,
            progress_every,
        } => simulate_trace(SimConfig {
            trace,
            switch: SwitchConfig { stages, sets, ways },
            progress_every,
        }),
    }
}

#[derive(Debug)]
struct RunConfig {
    edge_list: PathBuf,
    grid: u64,
    lanes: u64,
    epoch: u16,
    target: EdgeTarget,
    switch: SwitchConfig,
    limit_edges: Option<u64>,
    progress_every: Option<u64>,
}

#[derive(Debug)]
struct GenTraceConfig {
    edge_list: PathBuf,
    out: PathBuf,
    grid: u64,
    lanes: u64,
    epoch: u16,
    target: EdgeTarget,
    limit_edges: Option<u64>,
    progress_every: Option<u64>,
}

#[derive(Debug)]
struct SimConfig {
    trace: PathBuf,
    switch: SwitchConfig,
    progress_every: Option<u64>,
}

fn write_smoke_graph(out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create smoke parent {}", parent.display()))?;
    }

    let file =
        File::create(out).with_context(|| format!("create smoke graph {}", out.display()))?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "# ASTRA-Sim Phase 1 smoke graph")?;

    let hot_vertices = [130u64, 131, 190, 191, 4096, 4097, 8190, 8191];
    for src in 0..4096u64 {
        let dst = hot_vertices[(src as usize) % hot_vertices.len()];
        writeln!(writer, "{src} {dst}")?;
    }
    writer.flush()?;

    println!("mode=smoke");
    println!("out={}", out.display());
    println!("edges_written=4096");
    Ok(())
}

fn run_edge_stream(config: RunConfig) -> Result<()> {
    validate_lanes(config.lanes)?;
    validate_grid(config.grid)?;
    let owner_count = owner_count(config.grid)?;
    let mut switch = ReduceSwitch::new(config.switch, owner_count)?;

    let edge_stats = stream_edge_list(
        &config.edge_list,
        config.limit_edges,
        config.progress_every,
        |edge| {
            let vertex = select_vertex(edge, config.target);
            let packet = make_bfs_packet(vertex, config.epoch, config.lanes, config.grid)?;
            switch.process(packet)
        },
    )?;
    switch.drain()?;

    println!("--- EDGE STREAM STATS ---");
    println!("mode=edge_stream");
    println!("edge_list={}", config.edge_list.display());
    println!("target={}", config.target.name());
    println!("edges_read={}", edge_stats.edges_read);
    println!("packets_generated={}", edge_stats.edges_read);
    println!("skipped_lines={}", edge_stats.skipped_lines);
    println!();
    println!("--- FINAL SWITCH METRICS ---");
    print_metrics(switch.metrics(), config.switch, config.grid, config.lanes);
    Ok(())
}

fn gen_trace(config: GenTraceConfig) -> Result<()> {
    validate_lanes(config.lanes)?;
    validate_grid(config.grid)?;
    let header = TraceHeader::new(config.lanes, config.grid, config.epoch, config.target)?;
    let mut writer = TraceWriter::create(&config.out, header)?;

    let edge_stats = stream_edge_list(
        &config.edge_list,
        config.limit_edges,
        config.progress_every,
        |edge| {
            let vertex = select_vertex(edge, config.target);
            let packet = make_bfs_packet(vertex, config.epoch, config.lanes, config.grid)?;
            writer.write_packet(packet)
        },
    )?;
    let packet_count = writer.finish()?;

    println!("mode=gen_trace");
    println!("edge_list={}", config.edge_list.display());
    println!("trace={}", config.out.display());
    println!("target={}", config.target.name());
    println!("grid={}", config.grid);
    println!("lanes={}", config.lanes);
    println!("epoch={}", config.epoch);
    println!("edges_read={}", edge_stats.edges_read);
    println!("packets_generated={packet_count}");
    println!("skipped_lines={}", edge_stats.skipped_lines);
    Ok(())
}

fn simulate_trace(config: SimConfig) -> Result<()> {
    let mut reader = TraceReader::open(&config.trace)?;
    let header = reader.header();
    let owner_count = owner_count(header.grid)?;
    let mut switch = ReduceSwitch::new(config.switch, owner_count)?;
    let mut packets_read = 0u64;

    while let Some(packet) = reader.read_packet()? {
        switch.process(packet)?;
        packets_read = packets_read.saturating_add(1);
        if let Some(progress_every) = config.progress_every {
            if progress_every > 0 && packets_read.is_multiple_of(progress_every) {
                eprintln!("packets_read={packets_read}");
            }
        }
    }
    reader.finish()?;
    switch.drain()?;

    println!("--- TRACE STATS ---");
    println!("mode=trace");
    println!("trace={}", config.trace.display());
    println!("target={}", header.target.name());
    println!("trace_grid={}", header.grid);
    println!("trace_lanes={}", header.lanes);
    println!("trace_epoch={}", header.epoch);
    if header.packet_count == UNKNOWN_PACKET_COUNT {
        println!("trace_packet_count=unknown");
    } else {
        println!("trace_packet_count={}", header.packet_count);
    }
    println!("packets_read={packets_read}");
    println!();
    println!("--- FINAL SWITCH METRICS ---");
    print_metrics(switch.metrics(), config.switch, header.grid, header.lanes);
    Ok(())
}

fn select_vertex(edge: Edge, target: EdgeTarget) -> u64 {
    match target {
        EdgeTarget::Src => edge.src,
        EdgeTarget::Dst => edge.dst,
    }
}

fn print_metrics(metrics: &Metrics, switch: SwitchConfig, grid: u64, lanes: u64) {
    println!("grid={grid}");
    println!("lanes={lanes}");
    println!("stages={}", switch.stages);
    println!("sets={}", switch.sets);
    println!("ways={}", switch.ways);
    println!("entries={}", switch.entries());
    println!("packets_in={}", metrics.packets_in);
    println!("packets_out={}", metrics.packets_out);
    println!("table_hits={}", metrics.table_hits);
    println!("admitted={}", metrics.admitted);
    println!("bypassed={}", metrics.bypassed);
    println!("eviction_swaps={}", metrics.eviction_swaps);
    println!("eviction_flushes={}", metrics.eviction_flushes);
    println!("drained={}", metrics.drained);
    println!("packets_out_accounted={}", metrics.packets_out_accounted());
    println!("hit_rate={:.6}", metrics.hit_rate());
    println!("bypass_rate={:.6}", metrics.bypass_rate());
    println!("compression={:.6}", metrics.compression());
    println!("owner_queue_chips={}", metrics.owner_queue().len());
    println!("owner_queue_max={}", metrics.owner_queue_max());
    println!("owner_queue_mean={:.6}", metrics.owner_queue_mean());
}
