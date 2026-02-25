pub mod types;

use std::collections::HashMap;

use crate::ast::er_diagram::*;
use crate::ast::flowchart::{
    ArrowEnd, Direction, EdgeDef, FlowchartAst, LineStyle, NodeDef, NodeShape,
};
use crate::error::Result;
use crate::layout::flowchart;
use crate::layout::text_measure::TextMeasurer;

use self::types::*;

// ── Layout constants ────────────────────────────────────────

const ENTITY_PADDING_H: f64 = 12.0;
const ENTITY_PADDING_V: f64 = 6.0;
const HEADER_PADDING_V: f64 = 10.0;
const ROW_HEIGHT: f64 = 24.0;
const COL_GAP: f64 = 12.0;
const DIVIDER_HEIGHT: f64 = 1.0;

/// Compute layout positions for an ER diagram AST.
///
/// Strategy: convert the ER AST into a flowchart AST,
/// inject custom entity sizes before Sugiyama runs,
/// then convert back to ER-specific positioned types.
pub fn layout_er_diagram(
    ast: &ErDiagramAst,
    measurer: &TextMeasurer<'_>,
) -> Result<PositionedErDiagram> {
    // Pre-compute entity sizes and column widths
    let entity_sizes = compute_entity_sizes(ast, measurer);

    // 1. Convert ER AST to FlowchartAst
    let fc_ast = convert_to_flowchart_ast(ast);

    // 2. Replicate the flowchart layout pipeline with pre-layout size override
    use crate::layout::flowchart::{edge_routing, graph_builder, normalize};

    let class_defs = graph_builder::build_class_map(&fc_ast.class_defs);
    let all_nodes = graph_builder::collect_all_nodes(&fc_ast, &class_defs);
    let all_edges = graph_builder::collect_all_edges(&fc_ast);

    // Build dagre graph
    let (mut dagre_graph, mut node_data_map) = graph_builder::build_dagre_graph(
        &all_nodes,
        &all_edges,
        measurer,
        fc_ast.direction,
        &fc_ast,
    )?;

    // Override entity sizes in the dagre graph BEFORE layout
    override_entity_sizes_dagre(&mut dagre_graph, &mut node_data_map, &entity_sizes);

    // Run dagre layout
    dagre_rust::layout(&mut dagre_graph);

    // Build positioned nodes
    let mut positioned_nodes =
        flowchart::build_positioned_nodes_from_dagre(&dagre_graph, &node_data_map);

    // Extract bend points and route edges from dagre results
    let extraction = flowchart::extract_edge_data_from_dagre(&dagre_graph);

    let is_horizontal = matches!(
        fc_ast.direction,
        Direction::LeftToRight | Direction::RightToLeft
    );

    let mut positioned_edges = edge_routing::route_edges(
        &positioned_nodes,
        &all_edges,
        is_horizontal,
        &extraction.raw_points,
        &extraction.bend_points,
        &extraction.label_positions,
        &extraction.label_dimensions,
    );

    // Normalize coordinates
    let (width, height) = normalize::normalize_and_compute_bounds(
        &mut positioned_nodes,
        &mut positioned_edges,
        &mut Vec::new(),
    );

    // 3. Convert back to ER positioned types
    convert_from_flowchart_result(
        ast,
        positioned_nodes,
        positioned_edges,
        &entity_sizes,
        width,
        height,
    )
}

/// Pre-computed entity dimensions.
struct EntitySize {
    width: f64,
    height: f64,
    header_height: f64,
    col_widths: [f64; 4],
}

/// Compute sizes for all entities based on text measurement.
fn compute_entity_sizes(
    ast: &ErDiagramAst,
    measurer: &TextMeasurer<'_>,
) -> HashMap<String, EntitySize> {
    let mut sizes = HashMap::new();

    for entity in &ast.entities {
        // Measure header (entity name)
        let header_metrics = measurer.measure(&entity.id);
        let header_height = header_metrics.height + 2.0 * HEADER_PADDING_V;

        // Measure each attribute column
        let mut col_widths = [0.0f64; 4]; // type, name, key, comment

        for attr in &entity.attributes {
            let type_w = measurer.measure(&attr.type_name).width;
            let name_w = measurer.measure(&attr.name).width;
            let key_w = match attr.key {
                AttributeKey::PK => measurer.measure("PK").width,
                AttributeKey::FK => measurer.measure("FK").width,
                AttributeKey::UK => measurer.measure("UK").width,
                AttributeKey::None => 0.0,
            };
            let comment_w = attr
                .comment
                .as_ref()
                .map(|c| measurer.measure(c).width)
                .unwrap_or(0.0);

            col_widths[0] = col_widths[0].max(type_w);
            col_widths[1] = col_widths[1].max(name_w);
            col_widths[2] = col_widths[2].max(key_w);
            col_widths[3] = col_widths[3].max(comment_w);
        }

        // Total attributes width = sum of columns + gaps between non-zero columns
        let non_zero_cols = col_widths.iter().filter(|&&w| w > 0.0).count();
        let attrs_width: f64 = col_widths.iter().sum::<f64>()
            + if non_zero_cols > 1 {
                (non_zero_cols as f64 - 1.0) * COL_GAP
            } else {
                0.0
            };

        let content_width = header_metrics.width.max(attrs_width);
        let entity_width = content_width + 2.0 * ENTITY_PADDING_H;

        let n_rows = entity.attributes.len();
        let body_height = if n_rows > 0 {
            DIVIDER_HEIGHT + n_rows as f64 * ROW_HEIGHT + ENTITY_PADDING_V
        } else {
            0.0
        };
        let entity_height = header_height + body_height;

        sizes.insert(
            entity.id.clone(),
            EntitySize {
                width: entity_width.max(60.0), // minimum entity width
                height: entity_height.max(header_height),
                header_height,
                col_widths,
            },
        );
    }

    sizes
}

/// Override node sizes in the petgraph to match pre-computed entity sizes.
fn override_entity_sizes_dagre(
    g: &mut dagre_rust::LayoutGraph,
    node_data_map: &mut HashMap<String, crate::layout::flowchart::types::NodeData>,
    entity_sizes: &HashMap<String, EntitySize>,
) {
    for (id, size) in entity_sizes {
        if let Some(nl) = g.node_mut(id) {
            nl.width = size.width;
            nl.height = size.height;
        }
        if let Some(nd) = node_data_map.get_mut(id) {
            nd.width = size.width;
            nd.height = size.height;
        }
    }
}

/// Convert ER AST to FlowchartAst.
fn convert_to_flowchart_ast(ast: &ErDiagramAst) -> FlowchartAst {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Convert entities to rectangle nodes
    for entity in &ast.entities {
        nodes.push(NodeDef {
            id: entity.id.clone(),
            label: Some(entity.id.clone()),
            shape: NodeShape::Rectangle,
            class_shorthand: None,
        });
    }

    // Convert relationships to edges (no arrows — cardinality is handled in rendering)
    for rel in &ast.relationships {
        let line_style = match rel.relation_type {
            RelationType::Identifying => LineStyle::Solid,
            RelationType::NonIdentifying => LineStyle::Dotted,
        };

        edges.push(EdgeDef {
            from: rel.entity_a.clone(),
            to: rel.entity_b.clone(),
            line_style,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::None,
            label: rel.label.clone(),
            from_side: None,
            to_side: None,
        });
    }

    FlowchartAst {
        direction: Direction::TopToBottom,
        nodes,
        edges,
        subgraphs: Vec::new(),
        class_defs: Vec::new(),
        class_assignments: Vec::new(),
        style_overrides: Vec::new(),
    }
}

/// Convert flowchart positioned results back to ER positioned types.
fn convert_from_flowchart_result(
    ast: &ErDiagramAst,
    positioned_nodes: Vec<flowchart::PositionedNode>,
    positioned_edges: Vec<flowchart::PositionedEdge>,
    entity_sizes: &HashMap<String, EntitySize>,
    width: f64,
    height: f64,
) -> Result<PositionedErDiagram> {
    // Build entity lookup from AST
    let entity_map: HashMap<&str, &EntityDef> =
        ast.entities.iter().map(|e| (e.id.as_str(), e)).collect();

    // Build relationship lookup by (from, to) for cardinality info
    let rel_map: HashMap<(&str, &str), &RelationshipDef> = ast
        .relationships
        .iter()
        .map(|r| ((r.entity_a.as_str(), r.entity_b.as_str()), r))
        .collect();

    let mut entities = Vec::new();
    for node in &positioned_nodes {
        let ast_entity = entity_map.get(node.id.as_str());
        let size = entity_sizes.get(&node.id);

        entities.push(PositionedEntity {
            id: node.id.clone(),
            alias: ast_entity.and_then(|e| e.alias.clone()),
            attributes: ast_entity.map(|e| e.attributes.clone()).unwrap_or_default(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            header_height: size.map(|s| s.header_height).unwrap_or(30.0),
            col_widths: size.map(|s| s.col_widths).unwrap_or([0.0; 4]),
        });
    }

    let mut relationships = Vec::new();
    for edge in &positioned_edges {
        let rel = rel_map.get(&(edge.from_id.as_str(), edge.to_id.as_str()));

        relationships.push(PositionedRelationship {
            from_id: edge.from_id.clone(),
            to_id: edge.to_id.clone(),
            cardinality_from: rel.map(|r| r.cardinality_a).unwrap_or(Cardinality::OnlyOne),
            cardinality_to: rel.map(|r| r.cardinality_b).unwrap_or(Cardinality::OnlyOne),
            relation_type: rel
                .map(|r| r.relation_type)
                .unwrap_or(RelationType::Identifying),
            label: edge.label.clone(),
            label_x: edge.label_x,
            label_y: edge.label_y,
            label_width: edge.label_width,
            label_height: edge.label_height,
            points: edge.points.clone(),
        });
    }

    Ok(PositionedErDiagram {
        entities,
        relationships,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;
    use crate::layout::text_measure::TextMeasurer;

    fn make_measurer(provider: &FontProvider) -> TextMeasurer<'_> {
        let font = provider.font_ref().unwrap();
        TextMeasurer::new(font, 14.0)
    }

    fn make_simple_ast() -> ErDiagramAst {
        ErDiagramAst {
            entities: vec![
                EntityDef {
                    id: "Customer".into(),
                    alias: None,
                    attributes: vec![Attribute {
                        type_name: "int".into(),
                        name: "id".into(),
                        key: AttributeKey::PK,
                        comment: None,
                    }],
                },
                EntityDef {
                    id: "Order".into(),
                    alias: None,
                    attributes: vec![
                        Attribute {
                            type_name: "int".into(),
                            name: "id".into(),
                            key: AttributeKey::PK,
                            comment: None,
                        },
                        Attribute {
                            type_name: "int".into(),
                            name: "customer_id".into(),
                            key: AttributeKey::FK,
                            comment: Some("ref".into()),
                        },
                    ],
                },
            ],
            relationships: vec![RelationshipDef {
                entity_a: "Customer".into(),
                cardinality_a: Cardinality::OnlyOne,
                relation_type: RelationType::Identifying,
                cardinality_b: Cardinality::ZeroOrMore,
                entity_b: "Order".into(),
                label: Some("places".into()),
            }],
        }
    }

    #[test]
    fn convert_to_flowchart_ast_creates_nodes_for_entities() {
        let ast = make_simple_ast();
        let fc = convert_to_flowchart_ast(&ast);

        assert_eq!(fc.nodes.len(), 2);
        assert!(fc.nodes.iter().any(|n| n.id == "Customer"));
        assert!(fc.nodes.iter().any(|n| n.id == "Order"));
        // All nodes should be rectangles
        for node in &fc.nodes {
            assert_eq!(node.shape, NodeShape::Rectangle);
        }
    }

    #[test]
    fn convert_to_flowchart_ast_creates_edges_for_relationships() {
        let ast = make_simple_ast();
        let fc = convert_to_flowchart_ast(&ast);

        assert_eq!(fc.edges.len(), 1);
        assert_eq!(fc.edges[0].from, "Customer");
        assert_eq!(fc.edges[0].to, "Order");
        assert_eq!(fc.edges[0].label, Some("places".into()));
        // ER edges have no arrows (cardinality handled in rendering)
        assert_eq!(fc.edges[0].arrow_start, ArrowEnd::None);
        assert_eq!(fc.edges[0].arrow_end, ArrowEnd::None);
    }

    #[test]
    fn convert_to_flowchart_ast_identifying_is_solid() {
        let ast = ErDiagramAst {
            entities: vec![],
            relationships: vec![RelationshipDef {
                entity_a: "A".into(),
                cardinality_a: Cardinality::OnlyOne,
                relation_type: RelationType::Identifying,
                cardinality_b: Cardinality::OnlyOne,
                entity_b: "B".into(),
                label: None,
            }],
        };
        let fc = convert_to_flowchart_ast(&ast);
        assert_eq!(fc.edges[0].line_style, LineStyle::Solid);
    }

    #[test]
    fn convert_to_flowchart_ast_non_identifying_is_dotted() {
        let ast = ErDiagramAst {
            entities: vec![],
            relationships: vec![RelationshipDef {
                entity_a: "A".into(),
                cardinality_a: Cardinality::OnlyOne,
                relation_type: RelationType::NonIdentifying,
                cardinality_b: Cardinality::OnlyOne,
                entity_b: "B".into(),
                label: None,
            }],
        };
        let fc = convert_to_flowchart_ast(&ast);
        assert_eq!(fc.edges[0].line_style, LineStyle::Dotted);
    }

    #[test]
    fn convert_to_flowchart_ast_defaults() {
        let ast = make_simple_ast();
        let fc = convert_to_flowchart_ast(&ast);

        assert_eq!(fc.direction, Direction::TopToBottom);
        assert!(fc.subgraphs.is_empty());
        assert!(fc.class_defs.is_empty());
        assert!(fc.class_assignments.is_empty());
        assert!(fc.style_overrides.is_empty());
    }

    #[test]
    fn compute_entity_sizes_basic() {
        let ast = ErDiagramAst {
            entities: vec![EntityDef {
                id: "User".into(),
                alias: None,
                attributes: vec![
                    Attribute {
                        type_name: "int".into(),
                        name: "id".into(),
                        key: AttributeKey::PK,
                        comment: None,
                    },
                    Attribute {
                        type_name: "varchar".into(),
                        name: "email".into(),
                        key: AttributeKey::None,
                        comment: Some("user email".into()),
                    },
                ],
            }],
            relationships: vec![],
        };
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let sizes = compute_entity_sizes(&ast, &measurer);

        assert!(sizes.contains_key("User"));
        let size = &sizes["User"];
        assert!(size.width >= 60.0, "entity should have minimum width of 60");
        assert!(size.height > 0.0, "entity should have positive height");
        assert!(
            size.header_height > 0.0,
            "header should have positive height"
        );
        // type column should be wide enough for "varchar"
        assert!(size.col_widths[0] > 0.0, "type column should have width");
        // name column should be wide enough for "email"
        assert!(size.col_widths[1] > 0.0, "name column should have width");
        // comment column should be wide enough
        assert!(size.col_widths[3] > 0.0, "comment column should have width");
    }

    #[test]
    fn compute_entity_sizes_no_attributes() {
        let ast = ErDiagramAst {
            entities: vec![EntityDef {
                id: "Empty".into(),
                alias: None,
                attributes: vec![],
            }],
            relationships: vec![],
        };
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let sizes = compute_entity_sizes(&ast, &measurer);

        let size = &sizes["Empty"];
        // Height should just be the header height
        assert_eq!(size.height, size.header_height);
    }

    #[test]
    fn compute_entity_sizes_minimum_width() {
        let ast = ErDiagramAst {
            entities: vec![EntityDef {
                id: "X".into(), // very short name
                alias: None,
                attributes: vec![],
            }],
            relationships: vec![],
        };
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let sizes = compute_entity_sizes(&ast, &measurer);

        assert!(
            sizes["X"].width >= 60.0,
            "entity width should be at least 60.0"
        );
    }

    #[test]
    fn layout_er_diagram_basic() {
        let ast = make_simple_ast();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        assert_eq!(result.entities.len(), 2, "should have 2 entities");
        assert_eq!(result.relationships.len(), 1, "should have 1 relationship");
        assert!(result.width > 0.0, "width should be positive");
        assert!(result.height > 0.0, "height should be positive");

        // Check that entities have correct IDs
        let ids: Vec<&str> = result.entities.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"Customer"));
        assert!(ids.contains(&"Order"));
    }

    #[test]
    fn layout_er_diagram_preserves_cardinality() {
        let ast = make_simple_ast();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        let rel = &result.relationships[0];
        assert_eq!(rel.cardinality_from, Cardinality::OnlyOne);
        assert_eq!(rel.cardinality_to, Cardinality::ZeroOrMore);
        assert_eq!(rel.relation_type, RelationType::Identifying);
        assert_eq!(rel.label, Some("places".into()));
    }

    #[test]
    fn layout_er_diagram_preserves_attributes() {
        let ast = make_simple_ast();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        let order = result
            .entities
            .iter()
            .find(|e| e.id == "Order")
            .expect("should find Order entity");
        assert_eq!(order.attributes.len(), 2);
        assert_eq!(order.attributes[0].key, AttributeKey::PK);
        assert_eq!(order.attributes[1].key, AttributeKey::FK);
    }

    #[test]
    fn layout_er_diagram_entities_have_positive_dimensions() {
        let ast = make_simple_ast();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        for entity in &result.entities {
            assert!(
                entity.width > 0.0,
                "entity {} should have positive width",
                entity.id
            );
            assert!(
                entity.height > 0.0,
                "entity {} should have positive height",
                entity.id
            );
            assert!(
                entity.header_height > 0.0,
                "entity {} should have positive header height",
                entity.id
            );
        }
    }

    #[test]
    fn layout_er_diagram_relationships_have_points() {
        let ast = make_simple_ast();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        for rel in &result.relationships {
            assert!(
                rel.points.len() >= 2,
                "relationship {}->{} should have at least 2 points",
                rel.from_id,
                rel.to_id
            );
        }
    }

    #[test]
    fn layout_er_diagram_empty_inputs() {
        // Empty AST with no entities or relationships should still produce a result
        // (dagre requires at least some structure, so we just verify no panic
        // when there are entities but no relationships)
        let ast = ErDiagramAst {
            entities: vec![EntityDef {
                id: "Lone".into(),
                alias: None,
                attributes: vec![],
            }],
            relationships: vec![],
        };
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        assert_eq!(result.entities.len(), 1);
        assert!(result.relationships.is_empty());
    }

    #[test]
    fn layout_er_diagram_multiple_relationships() {
        let ast = ErDiagramAst {
            entities: vec![
                EntityDef {
                    id: "A".into(),
                    alias: None,
                    attributes: vec![],
                },
                EntityDef {
                    id: "B".into(),
                    alias: None,
                    attributes: vec![],
                },
                EntityDef {
                    id: "C".into(),
                    alias: None,
                    attributes: vec![],
                },
            ],
            relationships: vec![
                RelationshipDef {
                    entity_a: "A".into(),
                    cardinality_a: Cardinality::OnlyOne,
                    relation_type: RelationType::Identifying,
                    cardinality_b: Cardinality::OneOrMore,
                    entity_b: "B".into(),
                    label: Some("has".into()),
                },
                RelationshipDef {
                    entity_a: "B".into(),
                    cardinality_a: Cardinality::ZeroOrOne,
                    relation_type: RelationType::NonIdentifying,
                    cardinality_b: Cardinality::ZeroOrMore,
                    entity_b: "C".into(),
                    label: None,
                },
            ],
        };
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_er_diagram(&ast, &measurer).unwrap();

        assert_eq!(result.entities.len(), 3);
        assert_eq!(result.relationships.len(), 2);
    }
}
