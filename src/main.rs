mod bvgraph_reader;
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
use serde::Serialize;

use crate::{
    bvgraph_reader::stream_bvgraph,
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
        #[arg(long)]
        json: Option<PathBuf>,
    },
    RunBvgraph {
        #[arg(long)]
        basename: PathBuf,
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
        #[arg(long)]
        json: Option<PathBuf>,
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
    GenTraceBvgraph {
        #[arg(long)]
        basename: PathBuf,
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
        #[arg(long)]
        json: Option<PathBuf>,
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
            json,
        } => run_edge_stream(RunConfig {
            edge_list,
            grid,
            lanes,
            epoch,
            target: target.into(),
            switch: SwitchConfig { stages, sets, ways },
            limit_edges,
            progress_every,
            json,
        }),
        Commands::RunBvgraph {
            basename,
            grid,
            lanes,
            epoch,
            target,
            stages,
            sets,
            ways,
            limit_edges,
            progress_every,
            json,
        } => run_bvgraph_stream(BvRunConfig {
            basename,
            grid,
            lanes,
            epoch,
            target: target.into(),
            switch: SwitchConfig { stages, sets, ways },
            limit_edges,
            progress_every,
            json,
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
        Commands::GenTraceBvgraph {
            basename,
            out,
            grid,
            lanes,
            epoch,
            target,
            limit_edges,
            progress_every,
        } => gen_trace_bvgraph(BvGenTraceConfig {
            basename,
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
            json,
        } => simulate_trace(SimConfig {
            trace,
            switch: SwitchConfig { stages, sets, ways },
            progress_every,
            json,
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
    json: Option<PathBuf>,
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
struct BvRunConfig {
    basename: PathBuf,
    grid: u64,
    lanes: u64,
    epoch: u16,
    target: EdgeTarget,
    switch: SwitchConfig,
    limit_edges: Option<u64>,
    progress_every: Option<u64>,
    json: Option<PathBuf>,
}

#[derive(Debug)]
struct BvGenTraceConfig {
    basename: PathBuf,
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
    json: Option<PathBuf>,
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
    if let Some(json) = config.json {
        let report = JsonReport::edge_stream(
            JsonCommon::new(
                config.edge_list.display().to_string(),
                config.target.name(),
                config.grid,
                config.lanes,
                config.switch,
            ),
            JsonEdgeStreamStats {
                edges_read: edge_stats.edges_read,
                skipped_lines: edge_stats.skipped_lines,
            },
            switch.metrics(),
        );
        write_json_report(&json, &report)?;
    }
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

fn run_bvgraph_stream(config: BvRunConfig) -> Result<()> {
    validate_lanes(config.lanes)?;
    validate_grid(config.grid)?;
    let owner_count = owner_count(config.grid)?;
    let mut switch = ReduceSwitch::new(config.switch, owner_count)?;

    let graph_stats = stream_bvgraph(
        &config.basename,
        config.limit_edges,
        config.progress_every,
        |edge| {
            let vertex = select_vertex(edge, config.target);
            let packet = make_bfs_packet(vertex, config.epoch, config.lanes, config.grid)?;
            switch.process(packet)
        },
    )?;
    switch.drain()?;

    println!("--- BVGRAPH STREAM STATS ---");
    println!("mode=bvgraph_stream");
    println!("basename={}", config.basename.display());
    println!("target={}", config.target.name());
    println!("nodes={}", graph_stats.nodes);
    match graph_stats.arcs_hint {
        Some(arcs) => println!("arcs_hint={arcs}"),
        None => println!("arcs_hint=unknown"),
    }
    println!("edges_read={}", graph_stats.edges_read);
    println!("packets_generated={}", graph_stats.edges_read);
    println!();
    println!("--- FINAL SWITCH METRICS ---");
    print_metrics(switch.metrics(), config.switch, config.grid, config.lanes);
    if let Some(json) = config.json {
        let report = JsonReport::bvgraph_stream(
            JsonCommon::new(
                config.basename.display().to_string(),
                config.target.name(),
                config.grid,
                config.lanes,
                config.switch,
            ),
            JsonBvGraphStats {
                nodes: graph_stats.nodes,
                arcs_hint: graph_stats.arcs_hint,
                edges_read: graph_stats.edges_read,
            },
            switch.metrics(),
        );
        write_json_report(&json, &report)?;
    }
    Ok(())
}

fn gen_trace_bvgraph(config: BvGenTraceConfig) -> Result<()> {
    validate_lanes(config.lanes)?;
    validate_grid(config.grid)?;
    let header = TraceHeader::new(config.lanes, config.grid, config.epoch, config.target)?;
    let mut writer = TraceWriter::create(&config.out, header)?;

    let graph_stats = stream_bvgraph(
        &config.basename,
        config.limit_edges,
        config.progress_every,
        |edge| {
            let vertex = select_vertex(edge, config.target);
            let packet = make_bfs_packet(vertex, config.epoch, config.lanes, config.grid)?;
            writer.write_packet(packet)
        },
    )?;
    let packet_count = writer.finish()?;

    println!("mode=gen_trace_bvgraph");
    println!("basename={}", config.basename.display());
    println!("trace={}", config.out.display());
    println!("target={}", config.target.name());
    println!("grid={}", config.grid);
    println!("lanes={}", config.lanes);
    println!("epoch={}", config.epoch);
    println!("nodes={}", graph_stats.nodes);
    match graph_stats.arcs_hint {
        Some(arcs) => println!("arcs_hint={arcs}"),
        None => println!("arcs_hint=unknown"),
    }
    println!("edges_read={}", graph_stats.edges_read);
    println!("packets_generated={packet_count}");
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
    if let Some(json) = config.json {
        let report = JsonReport::trace(
            JsonCommon::new(
                config.trace.display().to_string(),
                header.target.name(),
                header.grid,
                header.lanes,
                config.switch,
            ),
            JsonTraceStats {
                trace_packet_count: header.packet_count,
                packets_read,
            },
            switch.metrics(),
        );
        write_json_report(&json, &report)?;
    }
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
    println!("table_hits={}", metrics.table_hits());
    println!("external_hits={}", metrics.external_hits);
    println!("internal_merge_hits={}", metrics.internal_merge_hits);
    println!("admitted={}", metrics.admitted);
    println!("bypassed={}", metrics.bypassed);
    println!("eviction_swaps={}", metrics.eviction_swaps);
    println!("eviction_flushes={}", metrics.eviction_flushes);
    println!("drained={}", metrics.drained);
    println!("packets_out_accounted={}", metrics.packets_out_accounted());
    println!("hit_rate={:.6}", metrics.hit_rate());
    println!("external_hit_rate={:.6}", metrics.external_hit_rate());
    println!("total_merge_rate={:.6}", metrics.total_merge_rate());
    println!("bypass_rate={:.6}", metrics.bypass_rate());
    println!("compression={:.6}", metrics.compression());
    println!("owner_queue_chips={}", metrics.owner_queue().len());
    println!("owner_queue_max={}", metrics.owner_queue_max());
    println!("owner_queue_mean={:.6}", metrics.owner_queue_mean());
}

#[derive(Debug, Serialize)]
struct JsonReport {
    mode: &'static str,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edges_read: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_lines: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arcs_hint: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_packet_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packets_read: Option<u64>,
    grid: u64,
    lanes: u64,
    switch: JsonSwitchConfig,
    metrics: JsonMetrics,
}

#[derive(Debug, Serialize)]
struct JsonSwitchConfig {
    stages: usize,
    sets: usize,
    ways: usize,
    entries: usize,
}

#[derive(Debug, Serialize)]
struct JsonMetrics {
    packets_in: u64,
    packets_out: u64,
    table_hits: u64,
    external_hits: u64,
    internal_merge_hits: u64,
    admitted: u64,
    bypassed: u64,
    eviction_swaps: u64,
    eviction_flushes: u64,
    drained: u64,
    packets_out_accounted: u64,
    hit_rate: f64,
    external_hit_rate: f64,
    total_merge_rate: f64,
    bypass_rate: f64,
    compression: f64,
    owner_queue_chips: usize,
    owner_queue_max: u64,
    owner_queue_mean: f64,
}

#[derive(Debug)]
struct JsonCommon {
    input: String,
    target: String,
    grid: u64,
    lanes: u64,
    switch: SwitchConfig,
}

#[derive(Debug)]
struct JsonEdgeStreamStats {
    edges_read: u64,
    skipped_lines: u64,
}

#[derive(Debug)]
struct JsonBvGraphStats {
    nodes: usize,
    arcs_hint: Option<u64>,
    edges_read: u64,
}

#[derive(Debug)]
struct JsonTraceStats {
    trace_packet_count: u64,
    packets_read: u64,
}

impl JsonCommon {
    fn new(input: String, target: &str, grid: u64, lanes: u64, switch: SwitchConfig) -> Self {
        Self {
            input,
            target: target.to_owned(),
            grid,
            lanes,
            switch,
        }
    }
}

impl JsonReport {
    fn edge_stream(common: JsonCommon, stats: JsonEdgeStreamStats, metrics: &Metrics) -> Self {
        Self {
            mode: "edge_stream",
            input: common.input,
            target: Some(common.target),
            edges_read: Some(stats.edges_read),
            skipped_lines: Some(stats.skipped_lines),
            nodes: None,
            arcs_hint: None,
            trace_packet_count: None,
            packets_read: None,
            grid: common.grid,
            lanes: common.lanes,
            switch: JsonSwitchConfig::from_switch(common.switch),
            metrics: JsonMetrics::from_metrics(metrics),
        }
    }

    fn bvgraph_stream(common: JsonCommon, stats: JsonBvGraphStats, metrics: &Metrics) -> Self {
        Self {
            mode: "bvgraph_stream",
            input: common.input,
            target: Some(common.target),
            edges_read: Some(stats.edges_read),
            skipped_lines: None,
            nodes: Some(stats.nodes),
            arcs_hint: stats.arcs_hint,
            trace_packet_count: None,
            packets_read: None,
            grid: common.grid,
            lanes: common.lanes,
            switch: JsonSwitchConfig::from_switch(common.switch),
            metrics: JsonMetrics::from_metrics(metrics),
        }
    }

    fn trace(common: JsonCommon, stats: JsonTraceStats, metrics: &Metrics) -> Self {
        Self {
            mode: "trace",
            input: common.input,
            target: Some(common.target),
            edges_read: None,
            skipped_lines: None,
            nodes: None,
            arcs_hint: None,
            trace_packet_count: (stats.trace_packet_count != UNKNOWN_PACKET_COUNT)
                .then_some(stats.trace_packet_count),
            packets_read: Some(stats.packets_read),
            grid: common.grid,
            lanes: common.lanes,
            switch: JsonSwitchConfig::from_switch(common.switch),
            metrics: JsonMetrics::from_metrics(metrics),
        }
    }
}

impl JsonSwitchConfig {
    fn from_switch(switch: SwitchConfig) -> Self {
        Self {
            stages: switch.stages,
            sets: switch.sets,
            ways: switch.ways,
            entries: switch.entries(),
        }
    }
}

impl JsonMetrics {
    fn from_metrics(metrics: &Metrics) -> Self {
        Self {
            packets_in: metrics.packets_in,
            packets_out: metrics.packets_out,
            table_hits: metrics.table_hits(),
            external_hits: metrics.external_hits,
            internal_merge_hits: metrics.internal_merge_hits,
            admitted: metrics.admitted,
            bypassed: metrics.bypassed,
            eviction_swaps: metrics.eviction_swaps,
            eviction_flushes: metrics.eviction_flushes,
            drained: metrics.drained,
            packets_out_accounted: metrics.packets_out_accounted(),
            hit_rate: metrics.hit_rate(),
            external_hit_rate: metrics.external_hit_rate(),
            total_merge_rate: metrics.total_merge_rate(),
            bypass_rate: metrics.bypass_rate(),
            compression: metrics.compression(),
            owner_queue_chips: metrics.owner_queue().len(),
            owner_queue_max: metrics.owner_queue_max(),
            owner_queue_mean: metrics.owner_queue_mean(),
        }
    }
}

fn write_json_report(path: &Path, report: &JsonReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create JSON report parent {}", parent.display()))?;
    }

    let file =
        File::create(path).with_context(|| format!("create JSON report {}", path.display()))?;
    serde_json::to_writer_pretty(file, report)
        .with_context(|| format!("write JSON report {}", path.display()))?;
    println!("json={}", path.display());
    Ok(())
}
