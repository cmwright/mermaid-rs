/// A CSS-like color value.
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Hex(String),
    Named(String),
    None,
}

impl Color {
    pub fn to_css(&self) -> String {
        match self {
            Color::Hex(h) => h.clone(),
            Color::Named(n) => n.clone(),
            Color::None => "none".to_string(),
        }
    }
}

/// Inline style properties (CSS-like key-value pairs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleProperties {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: Option<f64>,
    pub stroke_dasharray: Option<String>,
    pub color: Option<Color>,
    pub font_size: Option<f64>,
    pub extra: Vec<(String, String)>,
}

impl StyleProperties {
    /// Merge another set of properties on top of this one.
    /// Values from `other` override values in `self`.
    pub fn merge(&self, other: &StyleProperties) -> StyleProperties {
        StyleProperties {
            fill: other.fill.clone().or_else(|| self.fill.clone()),
            stroke: other.stroke.clone().or_else(|| self.stroke.clone()),
            stroke_width: other.stroke_width.or(self.stroke_width),
            stroke_dasharray: other
                .stroke_dasharray
                .clone()
                .or_else(|| self.stroke_dasharray.clone()),
            color: other.color.clone().or_else(|| self.color.clone()),
            font_size: other.font_size.or(self.font_size),
            extra: {
                let mut merged = self.extra.clone();
                for (k, v) in &other.extra {
                    if let Some(existing) = merged.iter_mut().find(|(ek, _)| ek == k) {
                        existing.1 = v.clone();
                    } else {
                        merged.push((k.clone(), v.clone()));
                    }
                }
                merged
            },
        }
    }
}

/// Parse a style properties string like "fill:#f9f,stroke:#333,stroke-width:4px"
pub fn parse_style_string(s: &str) -> StyleProperties {
    let mut props = StyleProperties::default();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "fill" => props.fill = Some(parse_color(value)),
                "stroke" => props.stroke = Some(parse_color(value)),
                "stroke-width" => {
                    props.stroke_width = value.trim_end_matches("px").parse::<f64>().ok();
                }
                "stroke-dasharray" => {
                    props.stroke_dasharray = Some(value.to_string());
                }
                "color" => props.color = Some(parse_color(value)),
                "font-size" => {
                    props.font_size = value.trim_end_matches("px").parse::<f64>().ok();
                }
                _ => {
                    props.extra.push((key.to_string(), value.to_string()));
                }
            }
        }
    }
    props
}

fn parse_color(s: &str) -> Color {
    let s = s.trim();
    if s.starts_with('#') {
        Color::Hex(s.to_string())
    } else if s == "none" || s == "transparent" {
        Color::None
    } else {
        Color::Named(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Color::to_css
    // ---------------------------------------------------------------

    #[test]
    fn color_none_to_css() {
        assert_eq!(Color::None.to_css(), "none");
    }

    #[test]
    fn color_named_to_css() {
        assert_eq!(Color::Named("red".into()).to_css(), "red");
    }

    #[test]
    fn color_hex_to_css() {
        assert_eq!(Color::Hex("#fff".into()).to_css(), "#fff");
    }

    // ---------------------------------------------------------------
    // StyleProperties::merge – named fields
    // ---------------------------------------------------------------

    #[test]
    fn merge_overrides_fill_stroke_stroke_width() {
        let base = StyleProperties {
            fill: Some(Color::Named("blue".into())),
            stroke: Some(Color::Named("black".into())),
            stroke_width: Some(2.0),
            ..Default::default()
        };
        let other = StyleProperties {
            fill: Some(Color::Named("red".into())),
            stroke: Some(Color::Hex("#aaa".into())),
            stroke_width: Some(5.0),
            ..Default::default()
        };

        let merged = base.merge(&other);

        assert_eq!(merged.fill, Some(Color::Named("red".into())));
        assert_eq!(merged.stroke, Some(Color::Hex("#aaa".into())));
        assert_eq!(merged.stroke_width, Some(5.0));
        // base values that were not overridden stay None because other
        // didn't set them and base didn't either for these fields.
    }

    #[test]
    fn merge_keeps_base_when_other_is_none() {
        let base = StyleProperties {
            fill: Some(Color::Named("blue".into())),
            stroke_width: Some(3.0),
            color: Some(Color::Named("green".into())),
            font_size: Some(14.0),
            stroke_dasharray: Some("5 3".into()),
            ..Default::default()
        };
        let other = StyleProperties::default();

        let merged = base.merge(&other);

        assert_eq!(merged.fill, Some(Color::Named("blue".into())));
        assert_eq!(merged.stroke_width, Some(3.0));
        assert_eq!(merged.color, Some(Color::Named("green".into())));
        assert_eq!(merged.font_size, Some(14.0));
        assert_eq!(merged.stroke_dasharray, Some("5 3".into()));
    }

    // ---------------------------------------------------------------
    // StyleProperties::merge – extra field
    // ---------------------------------------------------------------

    #[test]
    fn merge_extra_overrides_existing_key_and_appends_new_key() {
        let base = StyleProperties {
            extra: vec![("a".into(), "1".into())],
            ..Default::default()
        };
        let other = StyleProperties {
            extra: vec![("a".into(), "2".into()), ("b".into(), "3".into())],
            ..Default::default()
        };

        let merged = base.merge(&other);

        // "a" was overridden from "1" to "2"
        assert_eq!(
            merged.extra,
            vec![("a".into(), "2".into()), ("b".into(), "3".into())]
        );
    }

    // ---------------------------------------------------------------
    // parse_style_string – individual branches
    // ---------------------------------------------------------------

    #[test]
    fn parse_stroke_dasharray() {
        let props = parse_style_string("stroke-dasharray:5 3");
        assert_eq!(props.stroke_dasharray, Some("5 3".into()));
    }

    #[test]
    fn parse_color_property() {
        let props = parse_style_string("color:red");
        assert_eq!(props.color, Some(Color::Named("red".into())));
    }

    #[test]
    fn parse_font_size() {
        let props = parse_style_string("font-size:16px");
        assert_eq!(props.font_size, Some(16.0));
    }

    #[test]
    fn parse_unknown_property_goes_to_extra() {
        let props = parse_style_string("opacity:0.5");
        assert_eq!(props.extra, vec![("opacity".into(), "0.5".into())]);
    }

    // ---------------------------------------------------------------
    // parse_color (private) tested through parse_style_string
    // ---------------------------------------------------------------

    #[test]
    fn parse_color_transparent_returns_none_variant() {
        let props = parse_style_string("fill:transparent");
        assert_eq!(props.fill, Some(Color::None));
    }

    #[test]
    fn parse_color_none_returns_none_variant() {
        let props = parse_style_string("fill:none");
        assert_eq!(props.fill, Some(Color::None));
    }

    // ---------------------------------------------------------------
    // parse_style_string – multiple properties at once
    // ---------------------------------------------------------------

    #[test]
    fn parse_multiple_properties() {
        let props =
            parse_style_string("fill:#f9f, stroke:#333, stroke-width:4px, font-size:12px, opacity:0.8");

        assert_eq!(props.fill, Some(Color::Hex("#f9f".into())));
        assert_eq!(props.stroke, Some(Color::Hex("#333".into())));
        assert_eq!(props.stroke_width, Some(4.0));
        assert_eq!(props.font_size, Some(12.0));
        assert_eq!(props.extra, vec![("opacity".into(), "0.8".into())]);
    }
}
