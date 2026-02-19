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

    /// Word-wrap text so no line exceeds `max_width` pixels.
    ///
    /// For each line (already split by `\n`), if the line width exceeds
    /// `max_width`, break at word boundaries. Single words longer than
    /// `max_width` are kept intact (never mid-word break).
    pub fn wrap_text(&self, text: &str, max_width: f64) -> String {
        let mut result_lines: Vec<String> = Vec::new();

        for line in text.split('\n') {
            let line_width = self.measure(line).width;
            if line_width <= max_width {
                result_lines.push(line.to_string());
                continue;
            }

            // Need to wrap this line at word boundaries
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.is_empty() {
                result_lines.push(String::new());
                continue;
            }

            let mut current_line = String::new();
            for word in &words {
                if current_line.is_empty() {
                    // First word on the line — always add it (even if it exceeds max_width)
                    current_line = word.to_string();
                } else {
                    let candidate = format!("{} {}", current_line, word);
                    if self.measure(&candidate).width <= max_width {
                        current_line = candidate;
                    } else {
                        // Current line is full, start a new one
                        result_lines.push(current_line);
                        current_line = word.to_string();
                    }
                }
            }
            if !current_line.is_empty() {
                result_lines.push(current_line);
            }
        }

        result_lines.join("\n")
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
