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
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < s.len() {
        let rest = &s[i..];
        if rest.as_bytes().first() == Some(&b'<') {
            if let Some(close_rel) = rest.find('>') {
                let close_idx = i + close_rel;
                let tag = s[i + 1..close_idx].trim();
                let normalized_tag: String =
                    tag.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                if normalized_tag.eq_ignore_ascii_case("br")
                    || normalized_tag.eq_ignore_ascii_case("br/")
                {
                    result.push('\n');
                    i = close_idx + 1;
                    continue;
                }
            }
        }

        if let Some(ch) = rest.chars().next() {
            result.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
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
    fn test_normalize_br_uppercase() {
        assert_eq!(normalize_br("a<BR/>b"), "a\nb");
        assert_eq!(normalize_br("a<BR />b"), "a\nb");
        assert_eq!(normalize_br("a<BR>b"), "a\nb");
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

    #[test]
    fn test_has_html_with_tags() {
        assert!(has_html("<b>text</b>"));
    }

    #[test]
    fn test_has_html_plain_text() {
        assert!(!has_html("plain text"));
    }

    #[test]
    fn test_has_html_angle_brackets_without_proper_tags() {
        // has_html checks for both '<' and '>' present, so "< >" returns true
        assert!(has_html("< >"));
        // Only '<' without '>' returns false
        assert!(!has_html("just < but no close"));
        // Only '>' without '<' returns false
        assert!(!has_html("just > but no open"));
    }

    #[test]
    fn test_parse_segments_unknown_tag_i() {
        // <i> is not a recognized tag, so it gets skipped; text content is preserved
        let segs = parse_segments("<i>italic</i> text");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "italic text");
        assert!(!segs[0].bold);
    }

    #[test]
    fn test_parse_segments_unknown_tag_em() {
        // <em> is not a recognized tag, so it gets skipped; text content is preserved
        let segs = parse_segments("<em>emphasis</em>");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "emphasis");
        assert!(!segs[0].bold);
    }

    #[test]
    fn test_parse_segments_empty_input() {
        let segs = parse_segments("");
        assert!(segs.is_empty());
    }

    #[test]
    fn test_parse_segments_strong_tag() {
        // <strong> is treated the same as <b>
        let segs = parse_segments("<strong>bold</strong> normal");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "bold");
        assert!(segs[0].bold);
        assert_eq!(segs[1].text, " normal");
        assert!(!segs[1].bold);
    }

    #[test]
    fn test_parse_segments_text_before_bold_tag() {
        // When text precedes <b>, we must push the current segment before setting bold=true.
        // This exercises the if !current.is_empty() block in the "<b>" | "<strong>" branch.
        let segs = parse_segments("plain<b>Bold</b>");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "plain");
        assert!(!segs[0].bold);
        assert_eq!(segs[1].text, "Bold");
        assert!(segs[1].bold);
    }

    #[test]
    fn test_parse_segments_text_before_strong_tag() {
        // Same for <strong> - text before tag exercises the push+clear path
        let segs = parse_segments("prefix<strong>bold</strong> suffix");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].text, "prefix");
        assert!(!segs[0].bold);
        assert_eq!(segs[1].text, "bold");
        assert!(segs[1].bold);
        assert_eq!(segs[2].text, " suffix");
        assert!(!segs[2].bold);
    }
}
