//! Ranking module - assigns ranks to nodes.
//! Port of dagre's `rank/index.js`.

pub mod feasible_tree;
pub mod network_simplex;
pub mod util;

use crate::graph::LayoutGraph;
use crate::types::Ranker;

/// Assigns ranks to each node in the graph.
pub fn rank(g: &mut LayoutGraph) {
    let ranker = g.graph().ranker;

    match ranker {
        Ranker::NetworkSimplex => network_simplex::network_simplex(g),
        Ranker::TightTree => {
            util::longest_path(g);
            feasible_tree::feasible_tree(g);
        }
        Ranker::LongestPath => util::longest_path(g),
    }
}
