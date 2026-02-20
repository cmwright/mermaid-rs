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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{EdgeType, NodeShape};

    fn make_node(id: &str, x: f64, y: f64) -> PositionedNode {
        PositionedNode {
            id: id.to_string(),
            label: id.to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            x,
            y,
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_sg(id: &str, x: f64, y: f64) -> PositionedSubgraph {
        PositionedSubgraph {
            id: id.to_string(),
            label: Some(id.to_string()),
            x,
            y,
            width: 100.0,
            height: 80.0,
            style: Default::default(),
        }
    }

    #[test]
    fn test_min_from_subgraphs_only() {
        // min_x/min_y from subgraphs when nodes is empty (line 17-20)
        let mut nodes = vec![];
        let mut edges = vec![];
        let mut subgraphs = vec![
            make_sg("SG1", 10.0, 20.0),
            make_sg("SG2", -5.0, -10.0),
        ];
        let (w, h) = normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);
        assert!(subgraphs[1].x >= 0.0);
        assert!(subgraphs[1].y >= 0.0);
        assert!(w > 0.0 && h > 0.0);
    }

    #[test]
    fn test_empty_graph() {
        let (w, h) = normalize_and_compute_bounds(&mut [], &mut [], &mut []);
        assert!((w - 8.0).abs() < 0.1);
        assert!((h - 8.0).abs() < 0.1);
    }

    #[test]
    fn test_single_node_at_origin() {
        let mut nodes = vec![make_node("A", 20.0, 10.0)];
        let (w, h) = normalize_and_compute_bounds(&mut nodes, &mut [], &mut []);
        // Node at (20, 10), width=40, height=20
        // min_x = 20-20=0, min_y = 10-10=0
        // shift_x=0, shift_y=0
        // max_x = 20+20 = 40, max_y = 10+10 = 20
        assert!((w - 48.0).abs() < 0.1, "w={w}"); // 40 + 8
        assert!((h - 28.0).abs() < 0.1, "h={h}"); // 20 + 8
    }

    #[test]
    fn test_negative_coordinates_shifted() {
        let mut nodes = vec![make_node("A", -50.0, -30.0)];
        let mut edges = vec![];
        let mut subgraphs = vec![];
        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);
        // min_x = -50 - 20 = -70, shift_x = 70
        // new x = -50 + 70 = 20
        assert!(nodes[0].x >= 0.0, "node x should be non-negative after normalize");
        assert!(nodes[0].y >= 0.0, "node y should be non-negative after normalize");
    }

    #[test]
    fn test_edges_shifted_with_nodes() {
        let mut nodes = vec![
            make_node("A", -50.0, -30.0),
            make_node("B", 50.0, 70.0),
        ];
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            edge_type: EdgeType::SolidArrow,
            label: Some("test".into()),
            label_x: Some(0.0),
            label_y: Some(20.0),
            label_width: Some(30.0),
            label_height: Some(15.0),
            points: vec![(-30.0, -20.0), (30.0, 60.0)],
        }];
        let mut subgraphs = vec![];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);

        // All points and labels should be shifted by the same amount as nodes
        for point in &edges[0].points {
            assert!(point.0 >= -1.0 && point.1 >= -1.0, "edge point should be non-negative");
        }
        assert!(edges[0].label_x.unwrap() >= 0.0);
        assert!(edges[0].label_y.unwrap() >= 0.0);
    }

    #[test]
    fn test_subgraphs_shifted() {
        let mut nodes = vec![];
        let mut edges = vec![];
        let mut subgraphs = vec![make_sg("SG", -20.0, -10.0)];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);
        assert!(
            subgraphs[0].x >= -0.1,
            "subgraph x should be non-negative, got {}",
            subgraphs[0].x
        );
        assert!(
            subgraphs[0].y >= -0.1,
            "subgraph y should be non-negative, got {}",
            subgraphs[0].y
        );
    }

    #[test]
    fn test_bounds_account_for_subgraphs() {
        let mut nodes = vec![make_node("A", 20.0, 10.0)];
        let mut edges = vec![];
        // Subgraph extends further than the node
        let mut subgraphs = vec![make_sg("SG", 0.0, 0.0)]; // extends to (100, 80)

        let (w, h) = normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);
        // Bounds should cover the subgraph
        assert!(w >= 100.0 + 8.0, "width should cover subgraph, got {w}");
        assert!(h >= 80.0 + 8.0, "height should cover subgraph, got {h}");
    }

    #[test]
    fn test_edge_without_label() {
        let mut nodes = vec![
            make_node("A", 0.0, 0.0),
            make_node("B", 100.0, 100.0),
        ];
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            edge_type: EdgeType::SolidArrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(20.0, 10.0), (80.0, 90.0)],
        }];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut []);
        // Should not panic on None label coordinates
        assert!(edges[0].label_x.is_none());
        assert!(edges[0].label_y.is_none());
    }

    #[test]
    fn test_edge_label_x_only_shifted() {
        // Edge with label_x=Some, label_y=None exercises if let Some(ref mut lx) only
        let mut nodes = vec![
            make_node("A", -50.0, -30.0),
            make_node("B", 50.0, 70.0),
        ];
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            edge_type: EdgeType::SolidArrow,
            label: Some("x".into()),
            label_x: Some(-10.0),
            label_y: None,
            label_width: Some(20.0),
            label_height: None,
            points: vec![(-30.0, -20.0), (30.0, 60.0)],
        }];
        let mut subgraphs = vec![];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);

        // shift_x = 70, shift_y = 50
        assert!(edges[0].label_x.unwrap() >= 0.0, "label_x should be shifted");
        assert!(edges[0].label_y.is_none());
    }

    #[test]
    fn test_edge_label_both_shifted() {
        // Edge with both label_x and label_y -> both branches (lines 35-39)
        let mut nodes = vec![
            make_node("A", -50.0, -30.0),
            make_node("B", 50.0, 70.0),
        ];
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            edge_type: EdgeType::SolidArrow,
            label: Some("both".into()),
            label_x: Some(-10.0),
            label_y: Some(-15.0),
            label_width: Some(20.0),
            label_height: Some(15.0),
            points: vec![(-30.0, -20.0), (30.0, 60.0)],
        }];
        let mut subgraphs = vec![];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);

        assert!(edges[0].label_x.unwrap() >= 0.0);
        assert!(edges[0].label_y.unwrap() >= 0.0);
    }

    #[test]
    fn test_edge_label_y_only_shifted() {
        // Edge with label_x=None, label_y=Some exercises if let Some(ref mut ly) only
        let mut nodes = vec![
            make_node("A", -50.0, -30.0),
            make_node("B", 50.0, 70.0),
        ];
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            edge_type: EdgeType::SolidArrow,
            label: Some("y".into()),
            label_x: None,
            label_y: Some(-15.0),
            label_width: None,
            label_height: Some(15.0),
            points: vec![(-30.0, -20.0), (30.0, 60.0)],
        }];
        let mut subgraphs = vec![];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);

        assert!(edges[0].label_x.is_none());
        assert!(edges[0].label_y.unwrap() >= 0.0, "label_y should be shifted");
    }

    #[test]
    fn test_subgraphs_negative_coords_shift() {
        // Only subgraphs with negative coords; min comes from subgraphs
        let mut nodes = vec![];
        let mut edges = vec![];
        let mut subgraphs = vec![
            make_sg("SG1", -100.0, -50.0),
            make_sg("SG2", 10.0, 10.0),
        ];

        normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);

        assert!(subgraphs[0].x >= -0.1, "subgraph with negative x should be shifted");
        assert!(subgraphs[0].y >= -0.1, "subgraph with negative y should be shifted");
        assert!((subgraphs[1].x - 110.0).abs() < 0.1, "second subgraph shifted by 100");
        assert!((subgraphs[1].y - 60.0).abs() < 0.1, "second subgraph shifted by 50");
    }
}
