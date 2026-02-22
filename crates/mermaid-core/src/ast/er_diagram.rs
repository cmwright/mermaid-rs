/// The root AST for an ER diagram.
#[derive(Debug, Clone, Default)]
pub struct ErDiagramAst {
    pub entities: Vec<EntityDef>,
    pub relationships: Vec<RelationshipDef>,
}

/// An entity definition with optional attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityDef {
    pub id: String,
    pub alias: Option<String>,
    pub attributes: Vec<Attribute>,
}

/// A single attribute inside an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub type_name: String,
    pub name: String,
    pub key: AttributeKey,
    pub comment: Option<String>,
}

/// The key constraint type on an attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttributeKey {
    #[default]
    None,
    PK,
    FK,
    UK,
}

/// A relationship between two entities.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipDef {
    pub entity_a: String,
    pub cardinality_a: Cardinality,
    pub relation_type: RelationType,
    pub cardinality_b: Cardinality,
    pub entity_b: String,
    pub label: Option<String>,
}

/// Cardinality (multiplicity) of one side of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    OnlyOne,
    ZeroOrOne,
    OneOrMore,
    ZeroOrMore,
}

/// Whether the relationship is identifying (solid) or non-identifying (dashed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
    /// Solid line (`--`)
    Identifying,
    /// Dashed line (`..`)
    NonIdentifying,
}
