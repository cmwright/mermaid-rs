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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;

    #[test]
    fn test_measure_text_width() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let metrics = measurer.measure("Hello");
        assert!(metrics.width > 0.0, "Width should be positive");
        assert!(metrics.height > 0.0, "Height should be positive");

        // Empty string should have zero width
        let empty = measurer.measure("");
        assert!((empty.width - 0.0).abs() < 0.001, "Empty string should have zero width");
    }

    #[test]
    fn test_wrap_text_fits_one_line() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let short_text = "Hi";
        let result = measurer.wrap_text(short_text, 500.0);
        assert_eq!(result, "Hi", "Short text should not be wrapped");
    }

    #[test]
    fn test_wrap_text_wraps_at_spaces() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let long_text = "This is a fairly long sentence that should need wrapping at word boundaries";
        // Use a narrow max width to force wrapping
        let result = measurer.wrap_text(long_text, 80.0);
        let lines: Vec<&str> = result.split('\n').collect();
        assert!(
            lines.len() > 1,
            "Text should be wrapped into multiple lines, got {} line(s): {:?}",
            lines.len(),
            lines
        );
        // Verify no line exceeds max width (except possibly a single word)
        for line in &lines {
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.len() > 1 {
                let w = measurer.measure(line).width;
                assert!(
                    w <= 80.0 + 1.0, // small tolerance
                    "Multi-word line '{}' width {} should be <= max_width",
                    line,
                    w
                );
            }
        }
    }

    #[test]
    fn test_wrap_text_single_long_word() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let long_word = "Supercalifragilisticexpialidocious";
        let result = measurer.wrap_text(long_word, 50.0);
        // Single word longer than max_width should be kept intact (no mid-word break)
        assert_eq!(
            result, long_word,
            "Single long word should not be broken"
        );
    }

    #[test]
    fn test_wrap_text_preserves_existing_newlines() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let text = "Line one\nLine two\nLine three";
        let result = measurer.wrap_text(text, 500.0);
        // With large max_width, existing newlines should be preserved as-is
        assert_eq!(result, text);
    }

    #[test]
    fn test_wrap_text_empty_line() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        // A line that is only whitespace or empty after split
        let text = "Hello\n\nWorld";
        let result = measurer.wrap_text(text, 500.0);
        let lines: Vec<&str> = result.split('\n').collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_measure_multiline() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let text = "Hello\nWorld";
        let metrics = measurer.measure_multiline(text, 2.0);
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);

        // Should be taller than single line
        let single = measurer.measure("Hello");
        assert!(
            metrics.height > single.height,
            "Multi-line should be taller than single line"
        );
    }

    #[test]
    fn test_measure_multiline_empty() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let metrics = measurer.measure_multiline("", 2.0);
        assert!((metrics.width - 0.0).abs() < 0.001);
        assert!((metrics.height - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_measure_multiline_single_line() {
        // lines.len() == 1 case: total_height = line_height * 1 + line_spacing * 0
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        let text = "SingleLine";
        let metrics = measurer.measure_multiline(text, 2.0);
        let single = measurer.measure(text);
        assert!((metrics.width - single.width).abs() < 0.01);
        assert!((metrics.height - single.height).abs() < 0.01, "single line multiline height should match single measure");
    }

    #[test]
    fn test_measure_kerning_path() {
        // Multi-character text exercises prev_glyph_id = Some branch (kerning)
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        // "AV" is a common kerning pair; two chars ensure prev_glyph_id is Some for second char
        let two_char = measurer.measure("AV");
        let single_a = measurer.measure("A");
        let single_v = measurer.measure("V");
        // With kerning, width may be less than sum of individual widths
        assert!(two_char.width > 0.0);
        assert!(two_char.width <= single_a.width + single_v.width + 1.0);
    }

    #[test]
    fn test_wrap_text_line_whitespace_only() {
        // Line exceeding max_width but split_whitespace is empty (lines 63-64)
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);
        let spaces = " ".repeat(50);
        let result = measurer.wrap_text(&spaces, 10.0);
        // After split_whitespace there are no words, so result is an empty string
        assert!(result.is_empty());
    }

    #[test]
    fn test_measure_multiline_empty_string_zero_dims() {
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);
        let m = measurer.measure_multiline("", 2.0);
        assert_eq!(m.width, 0.0);
        assert_eq!(m.height, 0.0);
    }

    #[test]
    fn test_wrap_text_words_exceed_max_width_individually() {
        // Line that needs wrapping where each word exceeds max_width
        let fp = FontProvider::default_font();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, 14.0);

        // Both words longer than 30px at 14pt
        let text = "Supercalifragilistic Verylongwordexceeding";
        let result = measurer.wrap_text(text, 30.0);
        let lines: Vec<&str> = result.split('\n').collect();
        assert!(
            lines.len() >= 2,
            "words exceeding max_width individually should wrap, got {} lines: {:?}",
            lines.len(),
            lines
        );
        // First word on its own line, second on next
        assert!(lines[0].contains("Supercalifragilistic") || lines[0].contains("Verylongwordexceeding"));
    }
}
