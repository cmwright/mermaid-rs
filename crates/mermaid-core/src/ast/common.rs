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
