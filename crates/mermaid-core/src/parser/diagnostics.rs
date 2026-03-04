use pest::error::{Error as PestError, ErrorVariant, LineColLocation};
use pest::RuleType;

pub(crate) struct ParseContext<'a, R> {
    pub(crate) line: usize,
    pub(crate) col: usize,
    pub(crate) line_text: &'a str,
    pub(crate) token: Option<String>,
    pub(crate) expected_rules: Vec<R>,
}

pub(crate) fn build_parse_message<R, F>(
    source: &str,
    error: &PestError<R>,
    rule_label: F,
    hint_providers: &[fn(&ParseContext<'_, R>) -> Option<String>],
) -> String
where
    R: RuleType + Copy + Eq,
    F: Fn(R) -> Option<&'static str>,
{
    let (line, col) = match error.line_col {
        LineColLocation::Pos((l, c)) => (l, c),
        LineColLocation::Span((l, c), _) => (l, c),
    };

    let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let expected_rules = match &error.variant {
        ErrorVariant::ParsingError { positives, .. } => positives.clone(),
        ErrorVariant::CustomError { .. } => Vec::new(),
    };

    let context = ParseContext {
        line,
        col,
        line_text,
        token: token_near_column(line_text, col),
        expected_rules,
    };

    let mut message = match &error.variant {
        ErrorVariant::CustomError { message } => message.clone(),
        ErrorVariant::ParsingError { .. } => fallback_parsing_message(&context, &rule_label),
    };

    for provider in hint_providers {
        if let Some(hint) = provider(&context) {
            message.push_str(" Hint: ");
            message.push_str(&hint);
            break;
        }
    }

    message
}

fn fallback_parsing_message<R, F>(ctx: &ParseContext<'_, R>, rule_label: &F) -> String
where
    R: RuleType + Copy + Eq,
    F: Fn(R) -> Option<&'static str>,
{
    let token_text = ctx
        .token
        .as_ref()
        .map(|s| format!("`{}`", s))
        .unwrap_or_else(|| "this location".to_string());

    let expected_labels = expected_rule_labels(&ctx.expected_rules, rule_label);
    if expected_labels.is_empty() {
        return format!("Unexpected syntax near {}.", token_text);
    }

    format!(
        "Unexpected syntax near {}. Expected one of: {}.",
        token_text,
        expected_labels.join(", ")
    )
}

fn expected_rule_labels<R, F>(rules: &[R], rule_label: &F) -> Vec<&'static str>
where
    R: RuleType + Copy + Eq,
    F: Fn(R) -> Option<&'static str>,
{
    let mut out: Vec<&'static str> = Vec::new();
    for rule in rules {
        if let Some(label) = rule_label(*rule) {
            if !out.contains(&label) {
                out.push(label);
            }
        }
    }
    out
}

fn token_near_column(line_text: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line_text.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let mut idx = col.saturating_sub(1).min(chars.len().saturating_sub(1));

    if chars[idx].is_whitespace() {
        // Prefer the next non-whitespace token; if absent, look backward.
        let forward = (idx..chars.len()).find(|i| !chars[*i].is_whitespace());
        let backward = (0..=idx).rev().find(|i| !chars[*i].is_whitespace());
        idx = match (forward, backward) {
            (Some(i), _) => i,
            (None, Some(i)) => i,
            (None, None) => return None,
        };
    }

    if is_identifier_char(chars[idx]) {
        let mut start = idx;
        while start > 0 && is_identifier_char(chars[start - 1]) {
            start -= 1;
        }
        let mut end = idx;
        while end + 1 < chars.len() && is_identifier_char(chars[end + 1]) {
            end += 1;
        }
        return Some(chars[start..=end].iter().collect());
    }

    Some(chars[idx].to_string())
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

