//! dagre-rust: A 1:1 Rust port of the dagre JavaScript graph layout library.
//!
//! This library implements the Sugiyama-style layered graph layout algorithm,
//! producing x/y coordinates for nodes and edge waypoints.

pub mod acyclic;
pub mod add_border_segments;
pub mod coordinate_system;
pub mod data;
pub mod debug;
pub mod graph;
pub mod greedy_fas;
pub mod layout;
pub mod nesting_graph;
pub mod normalize;
pub mod order;
pub mod parent_dummy_chains;
pub mod position;
pub mod rank;
pub mod types;
pub mod util;

#[cfg(test)]
mod parity_harness;

// Re-export the main API
pub use graph::{Edge, Graph, GraphOptions, LayoutGraph};
pub use layout::{layout, layout_with_opts, LayoutOpts};
pub use types::*;
