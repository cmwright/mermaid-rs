use crate::ast::er_diagram::{Attribute, Cardinality, RelationType};

/// A positioned entity node.
#[derive(Debug, Clone)]
pub struct PositionedEntity {
    pub id: String,
    pub alias: Option<String>,
    pub attributes: Vec<Attribute>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Height of the header row (entity name).
    pub header_height: f64,
    /// Widths of attribute columns: [type, name, key, comment].
    pub col_widths: [f64; 4],
}

/// A positioned relationship (edge).
#[derive(Debug, Clone)]
pub struct PositionedRelationship {
    pub from_id: String,
    pub to_id: String,
    pub cardinality_from: Cardinality,
    pub cardinality_to: Cardinality,
    pub relation_type: RelationType,
    pub label: Option<String>,
    pub label_x: Option<f64>,
    pub label_y: Option<f64>,
    pub label_width: Option<f64>,
    pub label_height: Option<f64>,
    pub points: Vec<(f64, f64)>,
}

/// The full positioned ER diagram.
#[derive(Debug, Clone)]
pub struct PositionedErDiagram {
    pub entities: Vec<PositionedEntity>,
    pub relationships: Vec<PositionedRelationship>,
    pub width: f64,
    pub height: f64,
}
