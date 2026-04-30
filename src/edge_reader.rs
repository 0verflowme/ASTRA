use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub src: u64,
    pub dst: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeStreamStats {
    pub edges_read: u64,
    pub skipped_lines: u64,
}

pub fn stream_edge_list(
    path: &Path,
    limit_edges: Option<u64>,
    progress_every: Option<u64>,
    mut on_edge: impl FnMut(Edge) -> Result<()>,
) -> Result<EdgeStreamStats> {
    let file = File::open(path).with_context(|| format!("open edge list {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut stats = EdgeStreamStats::default();
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .with_context(|| format!("read edge list {}", path.display()))?;
        if bytes == 0 {
            break;
        }

        if let Some(limit) = limit_edges {
            if stats.edges_read >= limit {
                break;
            }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            stats.skipped_lines = stats.skipped_lines.saturating_add(1);
            continue;
        }

        let mut fields = trimmed.split_whitespace();
        let Some(src_raw) = fields.next() else {
            stats.skipped_lines = stats.skipped_lines.saturating_add(1);
            continue;
        };
        let Some(dst_raw) = fields.next() else {
            stats.skipped_lines = stats.skipped_lines.saturating_add(1);
            continue;
        };

        let Ok(src) = src_raw.parse::<u64>() else {
            stats.skipped_lines = stats.skipped_lines.saturating_add(1);
            continue;
        };
        let Ok(dst) = dst_raw.parse::<u64>() else {
            stats.skipped_lines = stats.skipped_lines.saturating_add(1);
            continue;
        };

        on_edge(Edge { src, dst })?;
        stats.edges_read = stats.edges_read.saturating_add(1);

        if let Some(progress_every) = progress_every {
            if progress_every > 0 && stats.edges_read.is_multiple_of(progress_every) {
                eprintln!("edges_read={}", stats.edges_read);
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn streams_edges_and_counts_skips() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("astra_edges_{nonce}.txt"));
        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "# comment").unwrap();
            writeln!(file).unwrap();
            writeln!(file, "1 2").unwrap();
            writeln!(file, "bad 3").unwrap();
            writeln!(file, "4 5 ignored").unwrap();
        }

        let mut edges = Vec::new();
        let stats = stream_edge_list(&path, None, None, |edge| {
            edges.push(edge);
            Ok(())
        })
        .unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(stats.edges_read, 2);
        assert_eq!(stats.skipped_lines, 3);
        assert_eq!(
            edges,
            vec![Edge { src: 1, dst: 2 }, Edge { src: 4, dst: 5 }]
        );
    }
}
