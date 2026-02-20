use crate::ast::common::Color;

/// Mermaid rendering theme — maps to Mermaid's built-in themes.
///
/// Universal fields live at the top level. Diagram-specific fields
/// are grouped into nested sub-structs.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Universal ────────────────────────────────────────────
    pub background: Color,
    pub line_color: Color,
    pub text_color: Color,
    pub font_family: String,
    pub font_size: f64,

    // ── Diagram-specific ─────────────────────────────────────
    pub flowchart: FlowchartTheme,
    pub sequence: SequenceTheme,
}

/// Flowchart-specific theme colours and sizes.
#[derive(Debug, Clone)]
pub struct FlowchartTheme {
    pub primary_color: Color,
    pub primary_border: Color,
    pub primary_text: Color,
    pub node_border_width: f64,
    pub edge_width: f64,
    pub arrowhead_size: f64,
    pub subgraph_fill: Color,
    pub subgraph_border: Color,
    pub subgraph_text: Color,
}

/// Sequence-diagram-specific theme colours.
#[derive(Debug, Clone)]
pub struct SequenceTheme {
    pub actor_fill: Color,
    pub actor_border: Color,
    pub actor_text: Color,
    pub note_fill: Color,
    pub note_border: Color,
    pub note_text: Color,
    pub activation_fill: Color,
    pub activation_border: Color,
    pub loop_fill: Color,
    pub loop_line: Color,
    pub label_box_fill: Color,
    pub lifeline_color: Color,
}

impl Default for Theme {
    /// Mermaid "default" theme.
    fn default() -> Self {
        Self {
            background: Color::Hex("#ffffff".into()),
            line_color: Color::Hex("#333333".into()),
            text_color: Color::Hex("#333333".into()),
            font_family: "'Hack', monospace".into(),
            font_size: 14.0,
            flowchart: FlowchartTheme {
                primary_color: Color::Hex("#ECECFF".into()),
                primary_border: Color::Hex("#9370DB".into()),
                primary_text: Color::Hex("#333333".into()),
                node_border_width: 1.0,
                edge_width: 1.0,
                arrowhead_size: 10.0,
                subgraph_fill: Color::Hex("#ebebff33".into()),
                subgraph_border: Color::Hex("#9370DB".into()),
                subgraph_text: Color::Hex("#333333".into()),
            },
            sequence: SequenceTheme {
                actor_fill: Color::Hex("#ECECFF".into()),
                actor_border: Color::Hex("#9370DB".into()),
                actor_text: Color::Hex("#333333".into()),
                note_fill: Color::Hex("#fff5ad".into()),
                note_border: Color::Hex("#aaaa33".into()),
                note_text: Color::Hex("#333333".into()),
                activation_fill: Color::Hex("#ECECFF".into()),
                activation_border: Color::Hex("#9370DB".into()),
                loop_fill: Color::Hex("#ebebff22".into()),
                loop_line: Color::Hex("#9370DB".into()),
                label_box_fill: Color::Hex("#ECECFF".into()),
                lifeline_color: Color::Hex("#ccccff".into()),
            },
        }
    }
}

impl Theme {
    /// Mermaid "dark" theme.
    pub fn dark() -> Self {
        Self {
            background: Color::Hex("#1f2020".into()),
            line_color: Color::Hex("#e0dfdf".into()),
            text_color: Color::Hex("#e0dfdf".into()),
            flowchart: FlowchartTheme {
                primary_color: Color::Hex("#1f2020".into()),
                primary_border: Color::Hex("#81B1DB".into()),
                primary_text: Color::Hex("#e0dfdf".into()),
                subgraph_fill: Color::Hex("#ffffff0d".into()),
                subgraph_border: Color::Hex("#a0c4e8".into()),
                subgraph_text: Color::Hex("#e0dfdf".into()),
                ..Self::default().flowchart
            },
            sequence: SequenceTheme {
                actor_fill: Color::Hex("#1f2020".into()),
                actor_border: Color::Hex("#81B1DB".into()),
                actor_text: Color::Hex("#e0dfdf".into()),
                note_fill: Color::Hex("#fff5ad".into()),
                note_border: Color::Hex("#aaaa33".into()),
                note_text: Color::Hex("#333333".into()),
                activation_fill: Color::Hex("#384252".into()),
                activation_border: Color::Hex("#81B1DB".into()),
                loop_fill: Color::Hex("#38425222".into()),
                loop_line: Color::Hex("#81B1DB".into()),
                label_box_fill: Color::Hex("#384252".into()),
                lifeline_color: Color::Hex("#4a5568".into()),
            },
            ..Self::default()
        }
    }

    /// Mermaid "forest" theme.
    pub fn forest() -> Self {
        Self {
            line_color: Color::Hex("#2b5329".into()),
            text_color: Color::Hex("#2b5329".into()),
            flowchart: FlowchartTheme {
                primary_color: Color::Hex("#cde498".into()),
                primary_border: Color::Hex("#13540c".into()),
                primary_text: Color::Hex("#13540c".into()),
                subgraph_fill: Color::Hex("#cde49833".into()),
                subgraph_border: Color::Hex("#13540c".into()),
                subgraph_text: Color::Hex("#13540c".into()),
                ..Self::default().flowchart
            },
            sequence: SequenceTheme {
                actor_fill: Color::Hex("#cde498".into()),
                actor_border: Color::Hex("#13540c".into()),
                actor_text: Color::Hex("#13540c".into()),
                note_fill: Color::Hex("#fff5ad".into()),
                note_border: Color::Hex("#aaaa33".into()),
                note_text: Color::Hex("#333333".into()),
                activation_fill: Color::Hex("#cde498".into()),
                activation_border: Color::Hex("#13540c".into()),
                loop_fill: Color::Hex("#cde49822".into()),
                loop_line: Color::Hex("#13540c".into()),
                label_box_fill: Color::Hex("#cde498".into()),
                lifeline_color: Color::Hex("#a8d68e".into()),
            },
            ..Self::default()
        }
    }

    /// Mermaid "neutral" theme.
    pub fn neutral() -> Self {
        Self {
            line_color: Color::Hex("#666666".into()),
            text_color: Color::Hex("#333333".into()),
            flowchart: FlowchartTheme {
                primary_color: Color::Hex("#f4f4f4".into()),
                primary_border: Color::Hex("#666666".into()),
                primary_text: Color::Hex("#333333".into()),
                subgraph_fill: Color::Hex("#f4f4f433".into()),
                subgraph_border: Color::Hex("#666666".into()),
                subgraph_text: Color::Hex("#333333".into()),
                ..Self::default().flowchart
            },
            sequence: SequenceTheme {
                actor_fill: Color::Hex("#f4f4f4".into()),
                actor_border: Color::Hex("#666666".into()),
                actor_text: Color::Hex("#333333".into()),
                note_fill: Color::Hex("#fff5ad".into()),
                note_border: Color::Hex("#aaaa33".into()),
                note_text: Color::Hex("#333333".into()),
                activation_fill: Color::Hex("#f4f4f4".into()),
                activation_border: Color::Hex("#666666".into()),
                loop_fill: Color::Hex("#f4f4f422".into()),
                loop_line: Color::Hex("#666666".into()),
                label_box_fill: Color::Hex("#f4f4f4".into()),
                lifeline_color: Color::Hex("#d9d9d9".into()),
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: extract the inner hex string from a `Color::Hex`.
    fn hex(color: &Color) -> &str {
        match color {
            Color::Hex(s) => s.as_str(),
            other => panic!("expected Color::Hex, got {:?}", other),
        }
    }

    #[test]
    fn forest_theme_has_forest_specific_colors() {
        let theme = Theme::forest();

        // Universal
        assert_eq!(hex(&theme.line_color), "#2b5329");
        assert_eq!(hex(&theme.text_color), "#2b5329");
        // background inherits from default via ..Self::default()
        assert_eq!(hex(&theme.background), "#ffffff");

        // Flowchart
        assert_eq!(hex(&theme.flowchart.primary_color), "#cde498");
        assert_eq!(hex(&theme.flowchart.primary_border), "#13540c");
        assert_eq!(hex(&theme.flowchart.primary_text), "#13540c");
        assert_eq!(hex(&theme.flowchart.subgraph_fill), "#cde49833");
        assert_eq!(hex(&theme.flowchart.subgraph_border), "#13540c");
        assert_eq!(hex(&theme.flowchart.subgraph_text), "#13540c");

        // Sequence
        assert_eq!(hex(&theme.sequence.actor_fill), "#cde498");
        assert_eq!(hex(&theme.sequence.actor_border), "#13540c");
        assert_eq!(hex(&theme.sequence.actor_text), "#13540c");
        assert_eq!(hex(&theme.sequence.activation_fill), "#cde498");
        assert_eq!(hex(&theme.sequence.activation_border), "#13540c");
        assert_eq!(hex(&theme.sequence.loop_fill), "#cde49822");
        assert_eq!(hex(&theme.sequence.loop_line), "#13540c");
        assert_eq!(hex(&theme.sequence.label_box_fill), "#cde498");
        assert_eq!(hex(&theme.sequence.lifeline_color), "#a8d68e");
    }

    #[test]
    fn neutral_theme_has_neutral_specific_colors() {
        let theme = Theme::neutral();

        // Universal
        assert_eq!(hex(&theme.line_color), "#666666");
        assert_eq!(hex(&theme.text_color), "#333333");
        assert_eq!(hex(&theme.background), "#ffffff");

        // Flowchart
        assert_eq!(hex(&theme.flowchart.primary_color), "#f4f4f4");
        assert_eq!(hex(&theme.flowchart.primary_border), "#666666");
        assert_eq!(hex(&theme.flowchart.primary_text), "#333333");
        assert_eq!(hex(&theme.flowchart.subgraph_fill), "#f4f4f433");
        assert_eq!(hex(&theme.flowchart.subgraph_border), "#666666");
        assert_eq!(hex(&theme.flowchart.subgraph_text), "#333333");

        // Sequence
        assert_eq!(hex(&theme.sequence.actor_fill), "#f4f4f4");
        assert_eq!(hex(&theme.sequence.actor_border), "#666666");
        assert_eq!(hex(&theme.sequence.actor_text), "#333333");
        assert_eq!(hex(&theme.sequence.activation_fill), "#f4f4f4");
        assert_eq!(hex(&theme.sequence.activation_border), "#666666");
        assert_eq!(hex(&theme.sequence.loop_fill), "#f4f4f422");
        assert_eq!(hex(&theme.sequence.loop_line), "#666666");
        assert_eq!(hex(&theme.sequence.label_box_fill), "#f4f4f4");
        assert_eq!(hex(&theme.sequence.lifeline_color), "#d9d9d9");
    }

    #[test]
    fn by_name_forest_returns_forest_theme() {
        let theme = Theme::by_name("forest");
        assert_eq!(hex(&theme.line_color), "#2b5329");
        assert_eq!(hex(&theme.flowchart.primary_color), "#cde498");
        assert_eq!(hex(&theme.sequence.actor_fill), "#cde498");
    }

    #[test]
    fn by_name_neutral_returns_neutral_theme() {
        let theme = Theme::by_name("neutral");
        assert_eq!(hex(&theme.line_color), "#666666");
        assert_eq!(hex(&theme.flowchart.primary_color), "#f4f4f4");
        assert_eq!(hex(&theme.sequence.actor_fill), "#f4f4f4");
    }

    #[test]
    fn by_name_unknown_returns_default_theme() {
        let theme = Theme::by_name("unknown");
        let default_theme = Theme::default();
        assert_eq!(hex(&theme.line_color), hex(&default_theme.line_color));
        assert_eq!(hex(&theme.text_color), hex(&default_theme.text_color));
        assert_eq!(hex(&theme.background), hex(&default_theme.background));
        assert_eq!(
            hex(&theme.flowchart.primary_color),
            hex(&default_theme.flowchart.primary_color)
        );
        assert_eq!(
            hex(&theme.sequence.actor_fill),
            hex(&default_theme.sequence.actor_fill)
        );
    }

    #[test]
    fn by_name_dark_returns_dark_theme() {
        let theme = Theme::by_name("dark");
        assert_eq!(hex(&theme.line_color), "#e0dfdf");
        assert_eq!(hex(&theme.background), "#1f2020");
        assert_eq!(hex(&theme.flowchart.primary_border), "#81B1DB");
        assert_eq!(hex(&theme.sequence.actor_border), "#81B1DB");
    }
}
