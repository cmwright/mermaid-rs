use crate::layout::flowchart::types::*;

/// Shift all positioned elements so everything has positive coordinates,
/// then compute the total bounding box.
pub fn normalize_and_compute_bounds(
    nodes: &mut [PositionedNode],
    edges: &mut [PositionedEdge],
    subgraphs: &mut [PositionedSubgraph],
) -> (f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;

    for node in nodes.iter() {
        min_x = min_x.min(node.x - node.width / 2.0);
        min_y = min_y.min(node.y - node.height / 2.0);
    }
    for sg in subgraphs.iter() {
        min_x = min_x.min(sg.x);
        min_y = min_y.min(sg.y);
    }

    if min_x.is_finite() && min_y.is_finite() {
        let shift_x = -min_x;
        let shift_y = -min_y;

        for node in nodes.iter_mut() {
            node.x += shift_x;
            node.y += shift_y;
        }
        for edge in edges.iter_mut() {
            for point in &mut edge.points {
                point.0 += shift_x;
                point.1 += shift_y;
            }
            if let Some(ref mut lx) = edge.label_x {
                *lx += shift_x;
            }
            if let Some(ref mut ly) = edge.label_y {
                *ly += shift_y;
            }
        }
        for sg in subgraphs.iter_mut() {
            sg.x += shift_x;
            sg.y += shift_y;
        }
    }

    let mut max_x = 0.0f64;
    let mut max_y = 0.0f64;

    for node in nodes.iter() {
        max_x = max_x.max(node.x + node.width / 2.0);
        max_y = max_y.max(node.y + node.height / 2.0);
    }
    for sg in subgraphs.iter() {
        max_x = max_x.max(sg.x + sg.width);
        max_y = max_y.max(sg.y + sg.height);
    }

    (max_x + 8.0, max_y + 8.0)
}
