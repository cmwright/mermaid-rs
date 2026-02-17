use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// Measured dimensions of a text string in pixels.
#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
}

pub struct TextMeasurer<'a> {
    font: FontRef<'a>,
    scale: PxScale,
}

impl<'a> TextMeasurer<'a> {
    pub fn new(font: FontRef<'a>, font_size: f32) -> Self {
        Self {
            font,
            scale: PxScale::from(font_size),
        }
    }

    /// Measure a single line of text, returning pixel width and height.
    pub fn measure(&self, text: &str) -> TextMetrics {
        let scaled = self.font.as_scaled(self.scale);
        let mut width = 0.0f32;
        let mut prev_glyph_id = None;

        for ch in text.chars() {
            let glyph_id = scaled.glyph_id(ch);
            if let Some(prev) = prev_glyph_id {
                width += scaled.kern(prev, glyph_id);
            }
            width += scaled.h_advance(glyph_id);
            prev_glyph_id = Some(glyph_id);
        }

        let height = scaled.height();
        TextMetrics {
            width: width as f64,
            height: height as f64,
        }
    }

    /// Measure multi-line text (split by newline).
    pub fn measure_multiline(&self, text: &str, line_spacing: f32) -> TextMetrics {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return TextMetrics {
                width: 0.0,
                height: 0.0,
            };
        }

        let mut max_width = 0.0f64;
        let scaled = self.font.as_scaled(self.scale);
        let line_height = scaled.height() as f64;

        for line in &lines {
            let m = self.measure(line);
            if m.width > max_width {
                max_width = m.width;
            }
        }

        let total_height =
            line_height * lines.len() as f64 + line_spacing as f64 * (lines.len() - 1) as f64;

        TextMetrics {
            width: max_width,
            height: total_height,
        }
    }
}
