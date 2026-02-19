/// Strip all HTML tags from text, preserving only text content.
pub fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Replace <br>, <br/>, <br /> (case-insensitive) with \n.
pub fn normalize_br(s: &str) -> String {
    let mut result = s.to_string();
    for pattern in &["<br/>", "<br />", "<br>", "<BR/>", "<BR />", "<BR>"] {
        result = result.replace(pattern, "\n");
    }
    result
}

/// Returns true if the label contains HTML-like tags.
pub fn has_html(s: &str) -> bool {
    s.contains('<') && s.contains('>')
}

/// A segment of formatted text within a label line.
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub bold: bool,
}

/// Parse a single line of label text into formatted segments.
pub fn parse_segments(line: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Collect tag
            let mut tag = String::from('<');
            for tc in chars.by_ref() {
                tag.push(tc);
                if tc == '>' {
                    break;
                }
            }
            let tag_lower = tag.to_lowercase();

            match tag_lower.as_str() {
                "<b>" | "<strong>" => {
                    if !current.is_empty() {
                        segments.push(TextSegment {
                            text: current.clone(),
                            bold,
                        });
                        current.clear();
                    }
                    bold = true;
                }
                "</b>" | "</strong>" => {
                    if !current.is_empty() {
                        segments.push(TextSegment {
                            text: current.clone(),
                            bold,
                        });
                        current.clear();
                    }
                    bold = false;
                }
                _ => {} // skip other tags like <i>, <em>, etc.
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        segments.push(TextSegment {
            text: current,
            bold,
        });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>Bold</b> text"), "Bold text");
        assert_eq!(strip_html_tags("no tags"), "no tags");
        assert_eq!(
            strip_html_tags("<b>Root OU</b><br/>id = 123"),
            "Root OUid = 123"
        );
    }

    #[test]
    fn test_normalize_br() {
        assert_eq!(normalize_br("a<br/>b<br>c"), "a\nb\nc");
        assert_eq!(normalize_br("a<br />b"), "a\nb");
    }

    #[test]
    fn test_parse_segments() {
        let segs = parse_segments("<b>Bold</b> normal");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "Bold");
        assert!(segs[0].bold);
        assert_eq!(segs[1].text, " normal");
        assert!(!segs[1].bold);
    }

    #[test]
    fn test_parse_segments_no_html() {
        let segs = parse_segments("plain text");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "plain text");
        assert!(!segs[0].bold);
    }
}
