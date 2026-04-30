use std::path::Path;

use anyhow::{Context, Result};
use lender::Lender;
use webgraph::{graphs::bvgraph::BvGraphSeq, prelude::SequentialLabeling};

use crate::edge_reader::Edge;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BvGraphStreamStats {
    pub nodes: usize,
    pub arcs_hint: Option<u64>,
    pub edges_read: u64,
}

pub fn stream_bvgraph(
    basename: &Path,
    limit_edges: Option<u64>,
    progress_every: Option<u64>,
    mut on_edge: impl FnMut(Edge) -> Result<()>,
) -> Result<BvGraphStreamStats> {
    let graph = BvGraphSeq::with_basename(basename)
        .load()
        .with_context(|| format!("load BVGraph basename {}", basename.display()))?;
    let mut stats = BvGraphStreamStats {
        nodes: graph.num_nodes(),
        arcs_hint: graph.num_arcs_hint(),
        edges_read: 0,
    };

    let mut iter = graph.iter();
    'nodes: while let Some((src, succ)) = iter.next() {
        for dst in succ {
            if let Some(limit) = limit_edges {
                if stats.edges_read >= limit {
                    break 'nodes;
                }
            }

            on_edge(Edge {
                src: src as u64,
                dst: dst as u64,
            })?;
            stats.edges_read = stats.edges_read.saturating_add(1);

            if let Some(progress_every) = progress_every {
                if progress_every > 0 && stats.edges_read.is_multiple_of(progress_every) {
                    eprintln!("edges_read={}", stats.edges_read);
                }
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsi_bitstream::prelude::BE;
    use webgraph::{graphs::vec_graph::VecGraph, prelude::BvComp};

    #[test]
    fn streams_small_bvgraph() {
        let graph = VecGraph::from_arcs([(0, 1), (0, 2), (1, 2)]);
        let tmp = tempfile::tempdir().unwrap();
        let basename = tmp.path().join("graph");
        BvComp::with_basename(&basename)
            .comp_graph::<BE>(&graph)
            .unwrap();

        let mut edges = Vec::new();
        let stats = stream_bvgraph(&basename, None, None, |edge| {
            edges.push(edge);
            Ok(())
        })
        .unwrap();

        assert_eq!(stats.nodes, 3);
        assert_eq!(stats.arcs_hint, Some(3));
        assert_eq!(stats.edges_read, 3);
        assert_eq!(
            edges,
            vec![
                Edge { src: 0, dst: 1 },
                Edge { src: 0, dst: 2 },
                Edge { src: 1, dst: 2 }
            ]
        );
    }
}
