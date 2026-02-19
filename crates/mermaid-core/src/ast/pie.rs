/// Top-level AST for a pie chart.
#[derive(Debug, Clone, Default)]
pub struct PieAst {
    pub title: Option<String>,
    pub slices: Vec<PieSlice>,
}

/// A single slice of the pie chart.
#[derive(Debug, Clone)]
pub struct PieSlice {
    pub label: String,
    pub value: f64,
}
