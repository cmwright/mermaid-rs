use crate::ast::common::Color;

/// Mermaid rendering theme — maps to Mermaid's built-in themes.
#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub primary_color: Color,
    pub primary_border: Color,
    pub primary_text: Color,
    pub secondary_color: Color,
    pub line_color: Color,
    pub text_color: Color,
    pub font_family: String,
    pub font_size: f64,
    pub node_border_width: f64,
    pub edge_width: f64,
    pub arrowhead_size: f64,
    pub subgraph_fill: Color,
    pub subgraph_border: Color,
    pub subgraph_text: Color,
}

impl Default for Theme {
    /// Mermaid "default" theme.
    fn default() -> Self {
        Self {
            background: Color::Hex("#ffffff".into()),
            primary_color: Color::Hex("#ECECFF".into()),
            primary_border: Color::Hex("#9370DB".into()),
            primary_text: Color::Hex("#333333".into()),
            secondary_color: Color::Hex("#ffffde".into()),
            line_color: Color::Hex("#333333".into()),
            text_color: Color::Hex("#333333".into()),
            font_family: "sans-serif".into(),
            font_size: 14.0,
            node_border_width: 2.0,
            edge_width: 2.0,
            arrowhead_size: 10.0,
            subgraph_fill: Color::Hex("#ebebff33".into()),
            subgraph_border: Color::Hex("#9370DB".into()),
            subgraph_text: Color::Hex("#333333".into()),
        }
    }
}

impl Theme {
    /// Mermaid "dark" theme.
    pub fn dark() -> Self {
        Self {
            background: Color::Hex("#1f2020".into()),
            primary_color: Color::Hex("#1f2020".into()),
            primary_border: Color::Hex("#81B1DB".into()),
            primary_text: Color::Hex("#e0dfdf".into()),
            secondary_color: Color::Hex("#384252".into()),
            line_color: Color::Hex("#e0dfdf".into()),
            text_color: Color::Hex("#e0dfdf".into()),
            subgraph_fill: Color::Hex("#384252".into()),
            subgraph_border: Color::Hex("#81B1DB".into()),
            subgraph_text: Color::Hex("#e0dfdf".into()),
            ..Self::default()
        }
    }

    /// Mermaid "forest" theme.
    pub fn forest() -> Self {
        Self {
            primary_color: Color::Hex("#cde498".into()),
            primary_border: Color::Hex("#13540c".into()),
            primary_text: Color::Hex("#13540c".into()),
            line_color: Color::Hex("#2b5329".into()),
            text_color: Color::Hex("#2b5329".into()),
            subgraph_fill: Color::Hex("#cde49833".into()),
            subgraph_border: Color::Hex("#13540c".into()),
            subgraph_text: Color::Hex("#13540c".into()),
            ..Self::default()
        }
    }

    /// Mermaid "neutral" theme.
    pub fn neutral() -> Self {
        Self {
            primary_color: Color::Hex("#f4f4f4".into()),
            primary_border: Color::Hex("#666666".into()),
            primary_text: Color::Hex("#333333".into()),
            line_color: Color::Hex("#666666".into()),
            text_color: Color::Hex("#333333".into()),
            subgraph_fill: Color::Hex("#f4f4f433".into()),
            subgraph_border: Color::Hex("#666666".into()),
            subgraph_text: Color::Hex("#333333".into()),
            ..Self::default()
        }
    }

    /// Get a theme by name.
    pub fn by_name(name: &str) -> Self {
        match name {
            "dark" => Self::dark(),
            "forest" => Self::forest(),
            "neutral" => Self::neutral(),
            _ => Self::default(),
        }
    }
}
