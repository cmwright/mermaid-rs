use pest::Parser;
use pest_derive::Parser;

use crate::ast::gantt::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/gantt.pest"]
struct GanttPestParser;

/// Parse a Mermaid Gantt chart source string into a GanttAst.
pub fn parse_gantt(source: &str) -> Result<GanttAst> {
    let pairs = GanttPestParser::parse(Rule::gantt_chart, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        MermaidError::Parse {
            line,
            col,
            message: format!("{}", e),
            source_snippet: Some(extract_snippet(source, line)),
        }
    })?;

    let mut ast = GanttAst::default();
    let mut current_section: Option<GanttSection> = None;
    let mut task_counter: u32 = 0;

    for pair in pairs {
        if pair.as_rule() != Rule::gantt_chart {
            continue;
        }
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::directive => {
                    parse_directive(&mut ast, inner)?;
                }
                Rule::section_line => {
                    // Push the current section and start a new one
                    if let Some(section) = current_section.take() {
                        ast.sections.push(section);
                    }
                    let name = inner
                        .into_inner()
                        .find(|p| p.as_rule() == Rule::section_name)
                        .map(|p| p.as_str().trim().to_string())
                        .unwrap_or_default();
                    current_section = Some(GanttSection {
                        name,
                        tasks: Vec::new(),
                    });
                }
                Rule::task_line => {
                    let task = parse_task_line(inner, &mut task_counter)?;
                    if let Some(ref mut section) = current_section {
                        section.tasks.push(task);
                    } else {
                        // Tasks before the first section go into an unnamed section
                        current_section = Some(GanttSection {
                            name: String::new(),
                            tasks: vec![task],
                        });
                    }
                }
                Rule::comment => {}
                _ => {}
            }
        }
    }

    // Push the last section
    if let Some(section) = current_section.take() {
        ast.sections.push(section);
    }

    Ok(ast)
}

fn parse_directive(ast: &mut GanttAst, pair: pest::iterators::Pair<'_, Rule>) -> Result<()> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::date_format_dir => {
            if let Some(val) = extract_dir_value(inner) {
                ast.date_format = val;
            }
        }
        Rule::axis_format_dir => {
            if let Some(val) = extract_dir_value(inner) {
                ast.axis_format = Some(val);
            }
        }
        Rule::title_dir => {
            if let Some(val) = extract_dir_value(inner) {
                ast.title = Some(val);
            }
        }
        Rule::excludes_dir => {
            if let Some(val) = extract_dir_value(inner) {
                for item in val.split(',') {
                    let trimmed = item.trim().to_string();
                    if !trimmed.is_empty() {
                        ast.excludes.push(trimmed);
                    }
                }
            }
        }
        Rule::includes_dir => {
            if let Some(val) = extract_dir_value(inner) {
                for item in val.split(',') {
                    let trimmed = item.trim().to_string();
                    if !trimmed.is_empty() {
                        ast.includes.push(trimmed);
                    }
                }
            }
        }
        Rule::tick_interval_dir => {
            if let Some(val) = extract_dir_value(inner) {
                ast.tick_interval = parse_tick_interval(&val);
            }
        }
        Rule::inclusive_end_dates_dir => {
            ast.inclusive_end_dates = true;
        }
        Rule::today_marker_dir => {
            if let Some(val) = extract_dir_value(inner) {
                if val.trim() == "off" {
                    ast.today_marker = TodayMarker::Off;
                } else {
                    ast.today_marker = TodayMarker::On;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn extract_dir_value(pair: pest::iterators::Pair<'_, Rule>) -> Option<String> {
    pair.into_inner()
        .find(|p| p.as_rule() == Rule::dir_value)
        .map(|p| p.as_str().trim().to_string())
}

fn parse_tick_interval(s: &str) -> Option<TickInterval> {
    let s = s.trim();
    // Try patterns like "1day", "1week", "1month", "2day", etc.
    // Also: "every 1day" or just "1d"
    let s = s.strip_prefix("every").unwrap_or(s).trim();

    // Parse count + unit
    let mut num_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            num_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if num_end == 0 {
        return None;
    }
    let count: u32 = s[..num_end].parse().ok()?;
    let unit_str = s[num_end..].trim().to_lowercase();

    let unit = match unit_str.as_str() {
        "d" | "day" | "days" => TickUnit::Day,
        "w" | "week" | "weeks" => TickUnit::Week,
        "m" | "month" | "months" => TickUnit::Month,
        _ => return None,
    };

    Some(TickInterval { count, unit })
}

fn parse_task_line(
    pair: pest::iterators::Pair<'_, Rule>,
    task_counter: &mut u32,
) -> Result<GanttTask> {
    let mut name = "";
    let mut raw_data = "";

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::task_name => {
                name = inner.as_str().trim();
            }
            Rule::task_data => {
                raw_data = inner.as_str().trim();
            }
            _ => {}
        }
    }

    parse_task_data(name, raw_data, task_counter)
}

/// Parse the raw task data string (everything after the colon).
///
/// Format: [tags,] [id,] start, end
/// Where tags can be: done, active, crit, milestone
/// Start can be: a date, "after <ids>", or omitted (previous task end)
/// End can be: a date, a duration (e.g. "3d"), or "until <ids>"
fn parse_task_data(name: &str, raw: &str, task_counter: &mut u32) -> Result<GanttTask> {
    let mut tags = TaskTags::default();
    let mut depends_on: Vec<String> = Vec::new();
    let mut done_reading_tags = false;

    // Keep only the first 3 non-tag fields since matching logic only uses these.
    let mut r0: Option<&str> = None;
    let mut r1: Option<&str> = None;
    let mut r2: Option<&str> = None;
    let mut remaining_len = 0usize;

    for field in raw.split(',').map(|s| s.trim()) {
        if !done_reading_tags {
            match field {
                "done" => {
                    tags.done = true;
                    continue;
                }
                "active" => {
                    tags.active = true;
                    continue;
                }
                "crit" => {
                    tags.crit = true;
                    continue;
                }
                "milestone" => {
                    tags.milestone = true;
                    continue;
                }
                _ => done_reading_tags = true,
            }
        }

        if let Some(dep_ids) = parse_depends_on_field(field) {
            depends_on.extend(dep_ids);
            continue;
        }

        match remaining_len {
            0 => r0 = Some(field),
            1 => r1 = Some(field),
            2 => r2 = Some(field),
            _ => {}
        }
        remaining_len += 1;
    }

    // Now interpret remaining fields:
    // 0 fields: no id, start=PrevEnd, end=Duration("1d") -- default
    // 1 field: could be id, duration, date, or "after x"
    // 2 fields: (start, end) or (id, something)
    // 3 fields: (id, start, end)

    let (id, start, end) = match remaining_len {
        0 => {
            // No data, use defaults
            (
                None,
                TaskStart::PrevEnd,
                TaskEnd::Duration("1d".to_string()),
            )
        }
        1 => {
            let f = r0.unwrap_or_default();
            if f.is_empty() {
                (
                    None,
                    TaskStart::PrevEnd,
                    TaskEnd::Duration("1d".to_string()),
                )
            } else if is_duration(f) {
                (None, TaskStart::PrevEnd, TaskEnd::Duration(f.to_string()))
            } else if f.starts_with("after ") {
                let ids = parse_id_list(&f[6..]);
                (
                    None,
                    TaskStart::After(ids),
                    TaskEnd::Duration("1d".to_string()),
                )
            } else {
                // Could be an id or a date as end
                // If it looks like a date, treat as end date
                // Otherwise treat as id
                if looks_like_date_or_start(f) {
                    (
                        None,
                        TaskStart::Date(f.to_string()),
                        TaskEnd::Duration("1d".to_string()),
                    )
                } else {
                    (
                        Some(f.to_string()),
                        TaskStart::PrevEnd,
                        TaskEnd::Duration("1d".to_string()),
                    )
                }
            }
        }
        2 => {
            let f0 = r0.unwrap_or_default();
            let f1 = r1.unwrap_or_default();

            if f0.starts_with("after ") {
                // (after ..., end)
                let ids = parse_id_list(&f0[6..]);
                let end = parse_end_field(f1);
                (None, TaskStart::After(ids), end)
            } else if f0.starts_with("until ") {
                let ids = parse_id_list(&f0[6..]);
                (None, TaskStart::PrevEnd, TaskEnd::Until(ids))
            } else if is_duration(f1) || looks_like_date_or_start(f1) || f1.starts_with("until ") {
                // (start, end) - no id
                let start = parse_start_field(f0);
                let end = parse_end_field(f1);
                (None, start, end)
            } else if is_duration(f0) {
                // (id, duration) -- id first, then duration as end
                (
                    Some(f0.to_string()),
                    TaskStart::PrevEnd,
                    TaskEnd::Duration(f0.to_string()),
                )
            } else {
                // Likely (id, start_or_end)
                // If the second field looks like a date or duration, first is id
                if !looks_like_id(f0) {
                    // first is start, second is end
                    let start = parse_start_field(f0);
                    let end = parse_end_field(f1);
                    (None, start, end)
                } else {
                    let end = parse_end_field(f1);
                    (Some(f0.to_string()), TaskStart::PrevEnd, end)
                }
            }
        }
        _ => {
            // 3+ fields: (id, start, end)
            let f0 = r0.unwrap_or_default();
            let f1 = r1.unwrap_or_default();
            let f2 = r2.unwrap_or_default();

            let id = Some(f0.to_string());
            let start = parse_start_field(f1);
            let end = parse_end_field(f2);
            (id, start, end)
        }
    };

    let final_id = if let Some(id) = id {
        Some(id)
    } else {
        *task_counter += 1;
        Some(format!("task{}", task_counter))
    };

    Ok(GanttTask {
        name: name.to_string(),
        tags,
        id: final_id,
        depends_on,
        start,
        end,
    })
}

fn parse_depends_on_field(field: &str) -> Option<Vec<String>> {
    let trimmed = field.trim();
    if trimmed.len() < "dependson".len() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("dependson") {
        return None;
    }
    let ids = trimmed["dependsOn".len()..].trim();
    if ids.is_empty() {
        return Some(Vec::new());
    }
    Some(parse_id_list(ids))
}

fn parse_id_list(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

fn is_duration(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // Duration patterns: 3d, 24h, 2w, 1m, 1M, 1y, 30s, 100ms
    // Must start with digits and end with a unit suffix
    let mut num_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            num_end = i + 1;
        } else {
            break;
        }
    }
    if num_end == 0 {
        return false;
    }
    let unit = &s[num_end..];
    matches!(unit, "d" | "h" | "w" | "m" | "M" | "y" | "s" | "ms")
}

fn looks_like_date_or_start(s: &str) -> bool {
    let s = s.trim();
    // A date typically starts with a digit and contains dashes, slashes, or is purely numeric
    if s.is_empty() {
        return false;
    }
    if s.starts_with("after ") || s.starts_with("until ") {
        return false;
    }
    // If it starts with a digit, it's likely a date
    s.chars().next().map_or(false, |c| c.is_ascii_digit())
}

fn looks_like_id(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // IDs are alphanumeric identifiers, not starting with digits (unless they do)
    // In mermaid, IDs are typically short strings like "des1", "a1"
    // They don't contain dashes in the middle typically (dates do)
    // If it contains a dash between digits, it's more likely a date
    if s.contains('-') && s.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        return false; // Likely a date like "2024-01-01"
    }
    // If it's purely numeric, it's likely a date or something else
    if s.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // Must be a valid identifier (letters, digits, underscores, hyphens)
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn parse_start_field(s: &str) -> TaskStart {
    let s = s.trim();
    if s.starts_with("after ") {
        TaskStart::After(parse_id_list(&s[6..]))
    } else if s.is_empty() {
        TaskStart::PrevEnd
    } else {
        TaskStart::Date(s.to_string())
    }
}

fn parse_end_field(s: &str) -> TaskEnd {
    let s = s.trim();
    if s.starts_with("until ") {
        TaskEnd::Until(parse_id_list(&s[6..]))
    } else if is_duration(s) {
        TaskEnd::Duration(s.to_string())
    } else {
        TaskEnd::Date(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_gantt() {
        let source = r#"gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
    A task :a1, 2014-01-01, 30d
    Another task :after a1, 20d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.title.as_deref(), Some("A Gantt Diagram"));
        assert_eq!(ast.date_format, "YYYY-MM-DD");
        assert_eq!(ast.sections.len(), 1);
        assert_eq!(ast.sections[0].name, "Section");
        assert_eq!(ast.sections[0].tasks.len(), 2);

        let t0 = &ast.sections[0].tasks[0];
        assert_eq!(t0.name, "A task");
        assert_eq!(t0.id.as_deref(), Some("a1"));
        assert!(matches!(&t0.start, TaskStart::Date(d) if d == "2014-01-01"));
        assert!(matches!(&t0.end, TaskEnd::Duration(d) if d == "30d"));

        let t1 = &ast.sections[0].tasks[1];
        assert_eq!(t1.name, "Another task");
        assert!(matches!(&t1.start, TaskStart::After(ids) if ids == &["a1"]));
        assert!(matches!(&t1.end, TaskEnd::Duration(d) if d == "20d"));
    }

    #[test]
    fn test_parse_tags() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Done task :done, des1, 2014-01-06, 2014-01-08
    Active task :active, des2, 2014-01-09, 3d
    Crit task :crit, done, des3, 2014-01-06, 24h
    Milestone :milestone, des4, 2014-01-12, 0d
"#;
        let ast = parse_gantt(source).unwrap();
        let tasks = &ast.sections[0].tasks;

        assert!(tasks[0].tags.done);
        assert!(!tasks[0].tags.active);
        assert_eq!(tasks[0].id.as_deref(), Some("des1"));

        assert!(tasks[1].tags.active);
        assert_eq!(tasks[1].id.as_deref(), Some("des2"));

        assert!(tasks[2].tags.crit);
        assert!(tasks[2].tags.done);
        assert_eq!(tasks[2].id.as_deref(), Some("des3"));

        assert!(tasks[3].tags.milestone);
        assert_eq!(tasks[3].id.as_deref(), Some("des4"));
    }

    #[test]
    fn test_parse_excludes() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    excludes weekends, monday
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.excludes, vec!["weekends", "monday"]);
    }

    #[test]
    fn test_parse_inclusive_end_dates() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    inclusiveEndDates
    section Test
    Task :2014-01-01, 2014-01-03
"#;
        let ast = parse_gantt(source).unwrap();
        assert!(ast.inclusive_end_dates);
    }

    #[test]
    fn test_parse_today_marker_off() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    todayMarker off
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.today_marker, TodayMarker::Off);
    }

    #[test]
    fn test_parse_multiple_sections() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Section A
    Task A1 :2014-01-01, 3d
    section Section B
    Task B1 :2014-01-04, 2d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.sections.len(), 2);
        assert_eq!(ast.sections[0].name, "Section A");
        assert_eq!(ast.sections[0].tasks.len(), 1);
        assert_eq!(ast.sections[1].name, "Section B");
        assert_eq!(ast.sections[1].tasks.len(), 1);
    }

    #[test]
    fn test_parse_comments() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    %% This is a comment
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.sections[0].tasks.len(), 1);
    }

    #[test]
    fn test_is_duration() {
        assert!(is_duration("3d"));
        assert!(is_duration("24h"));
        assert!(is_duration("2w"));
        assert!(is_duration("1M"));
        assert!(is_duration("100ms"));
        assert!(!is_duration("abc"));
        assert!(!is_duration("2014-01-01"));
        assert!(!is_duration(""));
    }

    #[test]
    fn test_parse_after_multiple_ids() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :a1, 2014-01-01, 3d
    Task B :b1, 2014-01-01, 5d
    Task C :after a1 b1, 2d
"#;
        let ast = parse_gantt(source).unwrap();
        let t2 = &ast.sections[0].tasks[2];
        assert!(matches!(&t2.start, TaskStart::After(ids) if ids == &["a1", "b1"]));
    }

    #[test]
    fn test_parse_axis_format() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.axis_format.as_deref(), Some("%Y-%m-%d"));
    }

    #[test]
    fn test_auto_id_generation() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :2014-01-01, 3d
    Task B :2014-01-04, 2d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.sections[0].tasks[0].id.as_deref(), Some("task1"));
        assert_eq!(ast.sections[0].tasks[1].id.as_deref(), Some("task2"));
    }

    #[test]
    fn test_parse_tick_interval() {
        assert!(parse_tick_interval("1day").is_some());
        let ti = parse_tick_interval("1day").unwrap();
        assert_eq!(ti.count, 1);
        assert_eq!(ti.unit, TickUnit::Day);

        let ti = parse_tick_interval("2week").unwrap();
        assert_eq!(ti.count, 2);
        assert_eq!(ti.unit, TickUnit::Week);
    }

    #[test]
    fn test_parse_depends_on_field() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :a1, 2014-01-01, 3d
    Task B :b1, 2014-01-04, 2d, dependsOn a1
"#;
        let ast = parse_gantt(source).unwrap();
        let tasks = &ast.sections[0].tasks;
        assert_eq!(tasks[0].depends_on, Vec::<String>::new());
        assert_eq!(tasks[1].depends_on, vec!["a1"]);
    }

    #[test]
    fn test_parse_depends_on_multiple_ids() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :a1, 2014-01-01, 3d
    Task B :b1, 2014-01-04, 2d
    Task C :c1, after a1, 5d, dependsOn a1 b1
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[2];
        assert_eq!(task.depends_on, vec!["a1", "b1"]);
        assert!(matches!(&task.start, TaskStart::After(ids) if ids == &["a1"]));
    }

    #[test]
    fn test_parse_axis_format_directive() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.axis_format.as_deref(), Some("%Y-%m-%d"));
    }

    #[test]
    fn test_parse_today_marker_off_directive() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    todayMarker off
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.today_marker, TodayMarker::Off);
    }

    #[test]
    fn test_parse_today_marker_custom_style() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    todayMarker stroke-dasharray:5
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        // Any non-"off" value leaves today_marker as On
        assert_eq!(ast.today_marker, TodayMarker::On);
    }

    #[test]
    fn test_parse_tick_interval_directive() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    tickInterval 1day
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert!(ast.tick_interval.is_some());
        let ti = ast.tick_interval.unwrap();
        assert_eq!(ti.count, 1);
        assert_eq!(ti.unit, TickUnit::Day);
    }

    #[test]
    fn test_parse_tick_interval_week() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    tickInterval 2week
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        let ti = ast.tick_interval.unwrap();
        assert_eq!(ti.count, 2);
        assert_eq!(ti.unit, TickUnit::Week);
    }

    #[test]
    fn test_parse_tick_interval_month() {
        let ti = parse_tick_interval("1month").unwrap();
        assert_eq!(ti.count, 1);
        assert_eq!(ti.unit, TickUnit::Month);
    }

    #[test]
    fn test_parse_tick_interval_with_every_prefix() {
        let ti = parse_tick_interval("every 1day").unwrap();
        assert_eq!(ti.count, 1);
        assert_eq!(ti.unit, TickUnit::Day);
    }

    #[test]
    fn test_parse_tick_interval_invalid() {
        assert!(parse_tick_interval("abc").is_none());
        assert!(parse_tick_interval("").is_none());
        assert!(parse_tick_interval("1xyz").is_none());
    }

    #[test]
    fn test_parse_task_with_after_dependency() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :a1, 2014-01-01, 3d
    Task B :after a1, 5d
"#;
        let ast = parse_gantt(source).unwrap();
        let t1 = &ast.sections[0].tasks[1];
        assert_eq!(t1.name, "Task B");
        assert!(matches!(&t1.start, TaskStart::After(ids) if ids == &["a1"]));
        assert!(matches!(&t1.end, TaskEnd::Duration(d) if d == "5d"));
    }

    #[test]
    fn test_parse_task_with_crit_active() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Critical Active Task :crit, active, 2024-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[0];
        assert!(task.tags.crit);
        assert!(task.tags.active);
        assert!(!task.tags.done);
        assert!(!task.tags.milestone);
        assert!(matches!(&task.start, TaskStart::Date(d) if d == "2024-01-01"));
        assert!(matches!(&task.end, TaskEnd::Duration(d) if d == "3d"));
    }

    #[test]
    fn test_parse_task_with_milestone() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Release :milestone, m1, 2024-01-15, 0d
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[0];
        assert_eq!(task.name, "Release");
        assert!(task.tags.milestone);
        assert_eq!(task.id.as_deref(), Some("m1"));
    }

    #[test]
    fn test_parse_task_with_until() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task A :a1, 2024-01-01, 3d
    Task B :2024-01-01, until a1
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[1];
        assert!(matches!(&task.end, TaskEnd::Until(ids) if ids == &["a1"]));
    }

    #[test]
    fn test_parse_task_before_section() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    Task before section :2014-01-01, 3d
    section Section A
    Task A1 :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.sections.len(), 2);
        assert_eq!(ast.sections[0].name, "");
        assert_eq!(ast.sections[0].tasks[0].name, "Task before section");
    }

    #[test]
    fn test_parse_task_with_id_start_end() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Task :des1, 2014-01-06, 2014-01-08
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[0];
        assert_eq!(task.id.as_deref(), Some("des1"));
        assert!(matches!(&task.start, TaskStart::Date(d) if d == "2014-01-06"));
        assert!(matches!(&task.end, TaskEnd::Date(d) if d == "2014-01-08"));
    }

    #[test]
    fn test_parse_includes_directive() {
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    includes monday
    section Test
    Task :2014-01-01, 3d
"#;
        let ast = parse_gantt(source).unwrap();
        assert_eq!(ast.includes, vec!["monday"]);
    }

    #[test]
    fn test_parse_task_no_data() {
        // A task with only tags and no other data
        let source = r#"gantt
    dateFormat YYYY-MM-DD
    section Test
    Done Task :done, active, crit, milestone
"#;
        let ast = parse_gantt(source).unwrap();
        let task = &ast.sections[0].tasks[0];
        assert!(task.tags.done);
        assert!(task.tags.active);
        assert!(task.tags.crit);
        assert!(task.tags.milestone);
        assert!(matches!(&task.start, TaskStart::PrevEnd));
        assert!(matches!(&task.end, TaskEnd::Duration(d) if d == "1d"));
    }
}
