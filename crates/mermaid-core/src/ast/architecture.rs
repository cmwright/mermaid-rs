use std::collections::HashMap;

use super::flowchart::{
    ArrowEnd, Direction, EdgeDef, EdgeSide, FlowchartAst, LineStyle, NodeDef, NodeShape,
    SubgraphDef,
};


/// The root AST for an architecture-beta diagram.
#[derive(Debug, Clone, Default)]
pub struct ArchitectureAst {
    pub groups: Vec<GroupDef>,
    pub services: Vec<ServiceDef>,
    pub junctions: Vec<JunctionDef>,
    pub edges: Vec<ArchEdge>,
}

impl ArchitectureAst {
    /// Convert this architecture AST into a flowchart AST so we can reuse
    /// the Sugiyama layout and flowchart SVG renderer.
    pub fn to_flowchart_ast(&self) -> FlowchartAst {
        // Build parent -> children maps
        let mut group_child_services: HashMap<Option<&str>, Vec<&ServiceDef>> = HashMap::new();
        let mut group_child_junctions: HashMap<Option<&str>, Vec<&JunctionDef>> = HashMap::new();
        let mut group_child_groups: HashMap<Option<&str>, Vec<&GroupDef>> = HashMap::new();

        for svc in &self.services {
            group_child_services
                .entry(svc.parent.as_deref())
                .or_default()
                .push(svc);
        }
        for junc in &self.junctions {
            group_child_junctions
                .entry(junc.parent.as_deref())
                .or_default()
                .push(junc);
        }
        for grp in &self.groups {
            group_child_groups
                .entry(grp.parent.as_deref())
                .or_default()
                .push(grp);
        }

        let top_subgraphs = build_subgraphs(
            None,
            &group_child_services,
            &group_child_junctions,
            &group_child_groups,
        );

        let top_nodes = build_nodes(None, &group_child_services, &group_child_junctions);

        let edges: Vec<EdgeDef> = self
            .edges
            .iter()
            .map(|e| EdgeDef {
                from: e.from.id.clone(),
                to: e.to.id.clone(),
                line_style: LineStyle::Solid,
                arrow_start: if e.arrow_start {
                    ArrowEnd::Arrow
                } else {
                    ArrowEnd::None
                },
                arrow_end: if e.arrow_end {
                    ArrowEnd::Arrow
                } else {
                    ArrowEnd::None
                },
                label: None,
                from_side: Some(match e.from.side {
                    PortSide::Top => EdgeSide::Top,
                    PortSide::Bottom => EdgeSide::Bottom,
                    PortSide::Left => EdgeSide::Left,
                    PortSide::Right => EdgeSide::Right,
                }),
                to_side: Some(match e.to.side {
                    PortSide::Top => EdgeSide::Top,
                    PortSide::Bottom => EdgeSide::Bottom,
                    PortSide::Left => EdgeSide::Left,
                    PortSide::Right => EdgeSide::Right,
                }),
            })
            .collect();

        FlowchartAst {
            direction: Direction::LeftToRight,
            nodes: top_nodes,
            edges,
            subgraphs: top_subgraphs,
            class_defs: Vec::new(),
            class_assignments: Vec::new(),
            style_overrides: Vec::new(),
        }
    }
}

fn build_nodes(
    parent: Option<&str>,
    group_child_services: &HashMap<Option<&str>, Vec<&ServiceDef>>,
    group_child_junctions: &HashMap<Option<&str>, Vec<&JunctionDef>>,
) -> Vec<NodeDef> {
    let mut nodes = Vec::new();

    if let Some(svcs) = group_child_services.get(&parent) {
        for svc in svcs {
            let label = if let Some(ref icon) = svc.icon {
                format!("{}\n{}", icon, svc.label)
            } else {
                svc.label.clone()
            };
            nodes.push(NodeDef {
                id: svc.id.clone(),
                label: Some(label),
                shape: NodeShape::RoundedRectangle,
                class_shorthand: None,
            });
        }
    }

    if let Some(juncs) = group_child_junctions.get(&parent) {
        for junc in juncs {
            nodes.push(NodeDef {
                id: junc.id.clone(),
                label: None,
                shape: NodeShape::Circle,
                class_shorthand: None,
            });
        }
    }

    nodes
}

fn build_subgraphs(
    parent: Option<&str>,
    group_child_services: &HashMap<Option<&str>, Vec<&ServiceDef>>,
    group_child_junctions: &HashMap<Option<&str>, Vec<&JunctionDef>>,
    group_child_groups: &HashMap<Option<&str>, Vec<&GroupDef>>,
) -> Vec<SubgraphDef> {
    let mut subgraphs = Vec::new();

    if let Some(grps) = group_child_groups.get(&parent) {
        for grp in grps {
            let child_nodes =
                build_nodes(Some(&grp.id), group_child_services, group_child_junctions);
            let child_subgraphs = build_subgraphs(
                Some(&grp.id),
                group_child_services,
                group_child_junctions,
                group_child_groups,
            );
            let label = if let Some(ref icon) = grp.icon {
                format!("{}\n{}", icon, grp.label)
            } else {
                grp.label.clone()
            };
            subgraphs.push(SubgraphDef {
                id: grp.id.clone(),
                label: Some(label),
                direction: None,
                nodes: child_nodes,
                edges: Vec::new(),
                subgraphs: child_subgraphs,
            });
        }
    }

    subgraphs
}

/// A group (container) declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDef {
    pub id: String,
    pub icon: Option<String>,
    pub label: String,
    pub parent: Option<String>,
}

/// A service (node) declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDef {
    pub id: String,
    pub icon: Option<String>,
    pub label: String,
    pub parent: Option<String>,
}

/// A junction (invisible routing node) declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct JunctionDef {
    pub id: String,
    pub parent: Option<String>,
}

/// An edge between two endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchEdge {
    pub from: EdgeEndpoint,
    pub to: EdgeEndpoint,
    pub arrow_start: bool,
    pub arrow_end: bool,
}

/// One endpoint of an edge: a node id, optional group modifier, and a port side.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeEndpoint {
    pub id: String,
    pub group_modifier: bool,
    pub side: PortSide,
}

/// Which side of a node a port attaches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSide {
    Top,
    Bottom,
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, Direction, EdgeSide, LineStyle, NodeShape};

    fn make_service(id: &str, icon: Option<&str>, label: &str, parent: Option<&str>) -> ServiceDef {
        ServiceDef {
            id: id.to_string(),
            icon: icon.map(|s| s.to_string()),
            label: label.to_string(),
            parent: parent.map(|s| s.to_string()),
        }
    }

    fn make_group(id: &str, icon: Option<&str>, label: &str, parent: Option<&str>) -> GroupDef {
        GroupDef {
            id: id.to_string(),
            icon: icon.map(|s| s.to_string()),
            label: label.to_string(),
            parent: parent.map(|s| s.to_string()),
        }
    }

    fn make_edge(from: &str, to: &str, arrow_start: bool, arrow_end: bool) -> ArchEdge {
        ArchEdge {
            from: EdgeEndpoint {
                id: from.to_string(),
                group_modifier: false,
                side: PortSide::Right,
            },
            to: EdgeEndpoint {
                id: to.to_string(),
                group_modifier: false,
                side: PortSide::Left,
            },
            arrow_start,
            arrow_end,
        }
    }

    #[test]
    fn service_with_icon_encodes_label() {
        let ast = ArchitectureAst {
            services: vec![make_service("s1", Some("server"), "Web Server", None)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.nodes.len(), 1);
        assert_eq!(fc.nodes[0].id, "s1");
        assert_eq!(fc.nodes[0].label.as_deref(), Some("server\nWeb Server"));
        assert_eq!(fc.nodes[0].shape, NodeShape::RoundedRectangle);
    }

    #[test]
    fn service_without_icon_plain_label() {
        let ast = ArchitectureAst {
            services: vec![make_service("s1", None, "Plain", None)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.nodes[0].label.as_deref(), Some("Plain"));
    }

    #[test]
    fn junction_becomes_circle_with_no_label() {
        let ast = ArchitectureAst {
            junctions: vec![JunctionDef {
                id: "j1".to_string(),
                parent: None,
            }],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.nodes.len(), 1);
        assert_eq!(fc.nodes[0].id, "j1");
        assert_eq!(fc.nodes[0].label, None);
        assert_eq!(fc.nodes[0].shape, NodeShape::Circle);
    }

    #[test]
    fn group_with_icon_encodes_subgraph_label() {
        let ast = ArchitectureAst {
            groups: vec![make_group("g1", Some("cloud"), "API Layer", None)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.subgraphs.len(), 1);
        assert_eq!(fc.subgraphs[0].id, "g1");
        assert_eq!(
            fc.subgraphs[0].label.as_deref(),
            Some("cloud\nAPI Layer")
        );
    }

    #[test]
    fn group_without_icon_plain_label() {
        let ast = ArchitectureAst {
            groups: vec![make_group("g1", None, "API Layer", None)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.subgraphs[0].label.as_deref(), Some("API Layer"));
    }

    #[test]
    fn service_inside_group_nests_correctly() {
        let ast = ArchitectureAst {
            groups: vec![make_group("g1", None, "Group", None)],
            services: vec![make_service("s1", Some("user"), "User", Some("g1"))],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        // Top-level should have no nodes (service is inside the group)
        assert!(fc.nodes.is_empty());
        assert_eq!(fc.subgraphs.len(), 1);
        assert_eq!(fc.subgraphs[0].nodes.len(), 1);
        assert_eq!(fc.subgraphs[0].nodes[0].id, "s1");
        assert_eq!(
            fc.subgraphs[0].nodes[0].label.as_deref(),
            Some("user\nUser")
        );
    }

    #[test]
    fn nested_groups() {
        let ast = ArchitectureAst {
            groups: vec![
                make_group("outer", Some("cloud"), "Cloud", None),
                make_group("inner", Some("server"), "Platform", Some("outer")),
            ],
            services: vec![make_service("s1", Some("cpu"), "Compute", Some("inner"))],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.subgraphs.len(), 1);
        let outer = &fc.subgraphs[0];
        assert_eq!(outer.id, "outer");
        assert!(outer.nodes.is_empty());
        assert_eq!(outer.subgraphs.len(), 1);

        let inner = &outer.subgraphs[0];
        assert_eq!(inner.id, "inner");
        assert_eq!(inner.nodes.len(), 1);
        assert_eq!(inner.nodes[0].id, "s1");
    }

    #[test]
    fn edge_arrow_conversion() {
        let ast = ArchitectureAst {
            services: vec![
                make_service("a", None, "A", None),
                make_service("b", None, "B", None),
            ],
            edges: vec![make_edge("a", "b", false, true)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.edges.len(), 1);
        assert_eq!(fc.edges[0].from, "a");
        assert_eq!(fc.edges[0].to, "b");
        assert_eq!(fc.edges[0].arrow_start, ArrowEnd::None);
        assert_eq!(fc.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(fc.edges[0].line_style, LineStyle::Solid);
        assert_eq!(fc.edges[0].label, None);
    }

    #[test]
    fn bidirectional_edge() {
        let ast = ArchitectureAst {
            services: vec![
                make_service("a", None, "A", None),
                make_service("b", None, "B", None),
            ],
            edges: vec![make_edge("a", "b", true, true)],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.edges[0].arrow_start, ArrowEnd::Arrow);
        assert_eq!(fc.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn direction_is_left_to_right() {
        let ast = ArchitectureAst::default();
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.direction, Direction::LeftToRight);
    }

    #[test]
    fn endpoint_port_sides_are_preserved_in_flowchart_edges() {
        let ast = ArchitectureAst {
            services: vec![
                make_service("a", None, "A", None),
                make_service("b", None, "B", None),
            ],
            edges: vec![ArchEdge {
                from: EdgeEndpoint {
                    id: "a".to_string(),
                    group_modifier: false,
                    side: PortSide::Bottom,
                },
                to: EdgeEndpoint {
                    id: "b".to_string(),
                    group_modifier: false,
                    side: PortSide::Top,
                },
                arrow_start: false,
                arrow_end: true,
            }],
            ..Default::default()
        };
        let fc = ast.to_flowchart_ast();
        assert_eq!(fc.edges.len(), 1);
        assert_eq!(fc.edges[0].from_side, Some(EdgeSide::Bottom));
        assert_eq!(fc.edges[0].to_side, Some(EdgeSide::Top));
    }
}
