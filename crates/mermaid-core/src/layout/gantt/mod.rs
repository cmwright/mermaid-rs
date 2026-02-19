use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;

use crate::ast::gantt::*;
use crate::error::{MermaidError, Result};
use crate::layout::text_measure::TextMeasurer;
use crate::render::theme::Theme;

// ── Constants ───────────────────────────────────────────────

const BAR_HEIGHT: f64 = 24.0;
const BAR_GAP: f64 = 4.0;
const SECTION_GAP: f64 = 8.0;
const LEFT_PADDING: f64 = 200.0;
const TOP_PADDING: f64 = 50.0;
const RIGHT_PADDING: f64 = 20.0;
const BOTTOM_PADDING: f64 = 20.0;
const MIN_CHART_WIDTH: f64 = 400.0;
const TASK_LABEL_PAD: f64 = 8.0;

// ── Output types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GanttLayout {
    pub width: f64,
    pub height: f64,
    pub title: Option<String>,
    pub title_y: f64,
    pub tasks: Vec<PositionedTask>,
    pub sections: Vec<PositionedSection>,
    pub grid_lines: Vec<GridLine>,
    pub dependency_edges: Vec<DependencyEdge>,
    pub today_x: Option<f64>,
    pub chart_x: f64,
    pub chart_y: f64,
    pub chart_width: f64,
    pub chart_height: f64,
    pub axis_format: String,
}

#[derive(Debug, Clone)]
pub struct PositionedTask {
    pub name: String,
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub tags: TaskTags,
    pub section_index: usize,
    pub is_milestone: bool,
    /// Measured width of the task label text in pixels.
    pub label_width: f64,
}

#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from_task_index: usize,
    pub to_task_index: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedSection {
    pub name: String,
    pub y_start: f64,
    pub y_end: f64,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct GridLine {
    pub x: f64,
    pub label: String,
    /// Whether to show the text label (false when labels would overlap).
    pub show_label: bool,
}

// ── Internal resolved task ──────────────────────────────────

#[derive(Debug, Clone)]
struct ResolvedTask {
    name: String,
    id: String,
    start: NaiveDateTime,
    end: NaiveDateTime,
    tags: TaskTags,
    _section_index: usize,
}

#[derive(Debug, Clone)]
enum DateRule {
    Weekends,
    Weekday(Weekday),
    ExactDate(NaiveDate),
}

#[derive(Debug, Clone)]
struct ExclusionRules {
    includes: Vec<DateRule>,
    excludes: Vec<DateRule>,
}

// ── Entry point ─────────────────────────────────────────────

pub fn layout_gantt(
    ast: &GanttAst,
    measurer: &TextMeasurer,
    _theme: &Theme,
) -> Result<GanttLayout> {
    let chrono_fmt = dayjs_to_chrono_format(&ast.date_format);
    let exclusion_rules = compile_exclusion_rules(&ast.excludes, &ast.includes);
    let axis_format = axis_format_str(ast);

    // Resolve all tasks to concrete dates
    let resolved = resolve_tasks(ast, &chrono_fmt, &exclusion_rules)?;

    if resolved.is_empty() {
        return Ok(GanttLayout {
            width: MIN_CHART_WIDTH,
            height: TOP_PADDING + BOTTOM_PADDING,
            title: ast.title.clone(),
            title_y: 20.0,
            tasks: Vec::new(),
            sections: Vec::new(),
            grid_lines: Vec::new(),
            dependency_edges: Vec::new(),
            today_x: None,
            chart_x: LEFT_PADDING,
            chart_y: TOP_PADDING,
            chart_width: MIN_CHART_WIDTH - LEFT_PADDING - RIGHT_PADDING,
            chart_height: 0.0,
            axis_format,
        });
    }

    // Determine time range
    let mut min_date = resolved[0].start;
    let mut max_date = resolved[0].end;
    for task in &resolved[1..] {
        if task.start < min_date {
            min_date = task.start;
        }
        if task.end > max_date {
            max_date = task.end;
        }
    }

    // Add a small padding to the time range
    let time_start = min_date - Duration::days(1);
    let time_end = max_date + Duration::days(1);
    let time_span = (time_end - time_start).num_seconds() as f64;

    // Measure all task labels
    let label_widths: Vec<f64> = resolved
        .iter()
        .map(|t| measurer.measure(&t.name).width)
        .collect();

    // chart_x is just a fixed left padding — labels now go inside/right of bars
    let chart_x = LEFT_PADDING;

    // Calculate chart dimensions
    let chart_width = MIN_CHART_WIDTH.max(time_span / 86400.0 * 20.0).min(2000.0);

    // Build sections for positioning
    let mut sections: Vec<PositionedSection> = Vec::new();

    // Position tasks
    let mut positioned_tasks = Vec::new();
    let mut y_cursor = TOP_PADDING;

    if ast.title.is_some() {
        y_cursor += 30.0;
    }

    let chart_y = y_cursor;
    let mut task_global_idx = 0;

    for (si, section) in ast.sections.iter().enumerate() {
        let section_y_start = y_cursor;

        if si > 0 {
            y_cursor += SECTION_GAP;
        }

        for ast_task in &section.tasks {
            if task_global_idx >= resolved.len() {
                break;
            }
            let rt = &resolved[task_global_idx];

            let task_offset_secs = (rt.start - time_start).num_seconds() as f64;
            let task_duration_secs = (rt.end - rt.start).num_seconds() as f64;

            let x = chart_x + (task_offset_secs / time_span) * chart_width;
            let w = (task_duration_secs / time_span) * chart_width;

            positioned_tasks.push(PositionedTask {
                name: rt.name.clone(),
                id: rt.id.clone(),
                x,
                y: y_cursor,
                width: w.max(1.0), // minimum 1px width
                height: BAR_HEIGHT,
                tags: rt.tags.clone(),
                section_index: si,
                is_milestone: ast_task.tags.milestone,
                label_width: label_widths[task_global_idx],
            });

            y_cursor += BAR_HEIGHT + BAR_GAP;
            task_global_idx += 1;
        }

        let section_y_end = y_cursor;
        sections.push(PositionedSection {
            name: section.name.clone(),
            y_start: section_y_start,
            y_end: section_y_end,
            index: si,
        });
    }

    let chart_height = y_cursor - chart_y;

    // Resolve visual dependency edges from explicit task depends_on fields.
    let mut dependency_edges = Vec::new();
    let mut seen_edges: HashSet<(usize, usize)> = HashSet::new();
    let mut id_to_index: HashMap<&str, usize> = HashMap::new();
    for (idx, task) in positioned_tasks.iter().enumerate() {
        if !task.id.is_empty() {
            id_to_index.insert(task.id.as_str(), idx);
        }
    }
    let mut to_task_index = 0usize;
    for section in &ast.sections {
        for task in &section.tasks {
            if to_task_index >= positioned_tasks.len() {
                break;
            }
            for dep_id in &task.depends_on {
                if let Some(&from_task_index) = id_to_index.get(dep_id.as_str()) {
                    if from_task_index == to_task_index {
                        continue;
                    }
                    if seen_edges.insert((from_task_index, to_task_index)) {
                        dependency_edges.push(DependencyEdge {
                            from_task_index,
                            to_task_index,
                        });
                    }
                }
            }
            to_task_index += 1;
        }
    }
    let dependency_edges =
        reduce_redundant_dependency_edges(positioned_tasks.len(), dependency_edges);

    // Generate grid lines
    let grid_lines = generate_grid_lines(
        ast,
        time_start,
        time_end,
        time_span,
        chart_x,
        chart_width,
        measurer,
    );

    // Today marker
    let today = chrono::Local::now().naive_local();
    let today_x = if ast.today_marker == TodayMarker::On && today >= time_start && today <= time_end
    {
        let offset = (today - time_start).num_seconds() as f64;
        Some(chart_x + (offset / time_span) * chart_width)
    } else {
        None
    };

    // Account for labels that are rendered outside bars on the right side.
    let max_label_right = positioned_tasks
        .iter()
        .filter_map(|task| {
            let label_fits_inside =
                task.label_width + TASK_LABEL_PAD * 2.0 <= task.width && !task.is_milestone;
            if label_fits_inside {
                return None;
            }
            let label_x = if task.is_milestone {
                task.x + task.height / 3.0 + TASK_LABEL_PAD
            } else {
                task.x + task.width + TASK_LABEL_PAD
            };
            Some(label_x + task.label_width)
        })
        .fold(0.0_f64, f64::max);

    let base_width = chart_x + chart_width + RIGHT_PADDING;
    let total_width = base_width.max(max_label_right + RIGHT_PADDING);
    let total_height = y_cursor + BOTTOM_PADDING;

    Ok(GanttLayout {
        width: total_width,
        height: total_height,
        title: ast.title.clone(),
        title_y: 20.0,
        tasks: positioned_tasks,
        sections,
        grid_lines,
        dependency_edges,
        today_x,
        chart_x,
        chart_y,
        chart_width,
        chart_height,
        axis_format,
    })
}

fn reduce_redundant_dependency_edges(
    task_count: usize,
    edges: Vec<DependencyEdge>,
) -> Vec<DependencyEdge> {
    if edges.len() < 2 || task_count == 0 {
        return edges;
    }

    let mut graph: DiGraphMap<usize, ()> = DiGraphMap::new();
    for i in 0..task_count {
        graph.add_node(i);
    }
    for edge in &edges {
        if edge.from_task_index < task_count && edge.to_task_index < task_count {
            graph.add_edge(edge.from_task_index, edge.to_task_index, ());
        }
    }

    // Transitive reduction is well-defined for DAGs; if there's a cycle,
    // keep all explicit edges to avoid surprising removals.
    let topo = match toposort(&graph, None) {
        Ok(order) => order,
        Err(_) => return edges,
    };

    let mut reachability: Vec<HashSet<usize>> = vec![HashSet::new(); task_count];
    for &node in topo.iter().rev() {
        for next in graph.neighbors_directed(node, petgraph::Direction::Outgoing) {
            reachability[node].insert(next);
            if next == node {
                continue;
            }
            if node < next {
                let (left, right) = reachability.split_at_mut(next);
                left[node].extend(right[0].iter().copied());
            } else {
                let (left, right) = reachability.split_at_mut(node);
                right[0].extend(left[next].iter().copied());
            }
        }
    }

    edges
        .into_iter()
        .filter(|edge| {
            !has_alternate_path(
                &graph,
                &reachability,
                edge.from_task_index,
                edge.to_task_index,
            )
        })
        .collect()
}

fn has_alternate_path(
    graph: &DiGraphMap<usize, ()>,
    reachability: &[HashSet<usize>],
    from: usize,
    to: usize,
) -> bool {
    if from >= reachability.len() || to >= reachability.len() {
        return false;
    }
    graph
        .neighbors_directed(from, petgraph::Direction::Outgoing)
        .any(|neighbor| neighbor != to && reachability[neighbor].contains(&to))
}

// ── Date resolution ─────────────────────────────────────────

fn resolve_tasks(
    ast: &GanttAst,
    chrono_fmt: &str,
    exclusion_rules: &ExclusionRules,
) -> Result<Vec<ResolvedTask>> {
    // Flatten all tasks with their section indices
    let mut all_tasks: Vec<(&GanttTask, usize)> = Vec::new();
    for (si, section) in ast.sections.iter().enumerate() {
        for task in &section.tasks {
            all_tasks.push((task, si));
        }
    }

    // Build id → index map
    let mut id_map: HashMap<String, usize> = HashMap::new();
    for (i, (task, _)) in all_tasks.iter().enumerate() {
        if let Some(ref id) = task.id {
            id_map.insert(id.clone(), i);
        }
    }

    // Resolve iteratively (up to 10 passes for forward references)
    let mut resolved: Vec<Option<(NaiveDateTime, NaiveDateTime)>> = vec![None; all_tasks.len()];

    for _pass in 0..10 {
        let mut changed = false;

        for i in 0..all_tasks.len() {
            if resolved[i].is_some() {
                continue;
            }

            let (task, _si) = &all_tasks[i];

            // Try to resolve start
            let start = match &task.start {
                TaskStart::Date(date_str) => parse_date(date_str, chrono_fmt).ok(),
                TaskStart::After(ids) => {
                    // Find the latest end time among referenced tasks
                    let mut latest: Option<NaiveDateTime> = None;
                    let mut all_resolved = true;
                    for ref_id in ids {
                        if let Some(&idx) = id_map.get(ref_id) {
                            if let Some((_, end)) = resolved[idx] {
                                latest = Some(match latest {
                                    Some(l) => l.max(end),
                                    None => end,
                                });
                            } else {
                                all_resolved = false;
                            }
                        }
                    }
                    if all_resolved {
                        latest
                    } else {
                        None
                    }
                }
                TaskStart::PrevEnd => {
                    if i == 0 {
                        // No previous task — use today or a default
                        Some(chrono::Local::now().naive_local())
                    } else if let Some((_, prev_end)) = resolved[i - 1] {
                        Some(prev_end)
                    } else {
                        None
                    }
                }
            };

            let start = match start {
                Some(s) => s,
                None => continue, // Can't resolve yet
            };

            // If the start date falls on an excluded day, push it forward
            let start = skip_excluded_start(start, exclusion_rules);

            // Try to resolve end
            let end = match &task.end {
                TaskEnd::Date(date_str) => {
                    let mut end_date = parse_date(date_str, chrono_fmt).map_err(|_| {
                        MermaidError::Layout(format!(
                            "Cannot parse end date '{}' for task '{}'",
                            date_str, task.name
                        ))
                    })?;
                    if ast.inclusive_end_dates {
                        end_date += Duration::days(1);
                    }
                    Some(end_date)
                }
                TaskEnd::Duration(dur_str) => {
                    let duration = parse_duration(dur_str)?;
                    let mut end = start + duration;
                    // Apply excludes for duration-based tasks
                    end = apply_excludes_to_end(start, end, duration, exclusion_rules);
                    Some(end)
                }
                TaskEnd::Until(ids) => {
                    let mut earliest: Option<NaiveDateTime> = None;
                    let mut all_resolved = true;
                    for ref_id in ids {
                        if let Some(&idx) = id_map.get(ref_id) {
                            if let Some((ref_start, _)) = resolved[idx] {
                                earliest = Some(match earliest {
                                    Some(e) => e.min(ref_start),
                                    None => ref_start,
                                });
                            } else {
                                all_resolved = false;
                            }
                        }
                    }
                    if all_resolved {
                        earliest
                    } else {
                        None
                    }
                }
            };

            let end = match end {
                Some(e) => e,
                None => continue,
            };

            resolved[i] = Some((start, end));
            changed = true;
        }

        if !changed {
            break;
        }
    }

    // Build final resolved tasks
    let mut result = Vec::new();
    for (i, (task, si)) in all_tasks.iter().enumerate() {
        let (start, end) = resolved[i].unwrap_or_else(|| {
            // Fallback: use today for unresolved tasks
            let now = chrono::Local::now().naive_local();
            (now, now + Duration::days(1))
        });

        result.push(ResolvedTask {
            name: task.name.clone(),
            id: task.id.clone().unwrap_or_default(),
            start,
            end,
            tags: task.tags.clone(),
            _section_index: *si,
        });
    }

    Ok(result)
}

// ── Date/time helpers ───────────────────────────────────────

/// Convert dayjs-style format string to chrono format string.
fn dayjs_to_chrono_format(fmt: &str) -> String {
    // Replace dayjs tokens with chrono tokens (order matters — longest first)
    fmt.replace("YYYY", "%Y")
        .replace("YY", "%y")
        .replace("MMMM", "%B")
        .replace("MMM", "%b")
        .replace("MM", "%m")
        .replace("DD", "%d")
        .replace("HH", "%H")
        .replace("mm", "%M")
        .replace("ss", "%S")
        .replace("X", "%s")
}

/// Parse a date string using a chrono format, returning a NaiveDateTime.
fn parse_date(date_str: &str, chrono_fmt: &str) -> Result<NaiveDateTime> {
    let date_str = date_str.trim();

    // Try parsing as NaiveDateTime first
    if let Ok(dt) = NaiveDateTime::parse_from_str(date_str, chrono_fmt) {
        return Ok(dt);
    }

    // Try parsing as NaiveDate (without time component)
    if let Ok(d) = NaiveDate::parse_from_str(date_str, chrono_fmt) {
        return Ok(d.and_hms_opt(0, 0, 0).unwrap());
    }

    // Try common fallback formats
    let fallback_formats = ["%Y-%m-%d", "%m/%d/%Y", "%d/%m/%Y", "%Y%m%d"];
    for ff in &fallback_formats {
        if let Ok(d) = NaiveDate::parse_from_str(date_str, ff) {
            return Ok(d.and_hms_opt(0, 0, 0).unwrap());
        }
    }

    Err(MermaidError::Layout(format!(
        "Cannot parse date '{}' with format '{}'",
        date_str, chrono_fmt
    )))
}

/// Parse a duration string like "3d", "24h", "2w", "1M".
fn parse_duration(dur_str: &str) -> Result<Duration> {
    let s = dur_str.trim();

    // Find where the number ends
    let mut num_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            num_end = i + 1;
        } else {
            break;
        }
    }

    if num_end == 0 {
        return Err(MermaidError::Layout(format!(
            "Invalid duration: '{}'",
            dur_str
        )));
    }

    let count: i64 = s[..num_end]
        .parse()
        .map_err(|_| MermaidError::Layout(format!("Invalid duration number: '{}'", dur_str)))?;

    let unit = &s[num_end..];
    let duration = match unit {
        "ms" => Duration::milliseconds(count),
        "s" => Duration::seconds(count),
        "m" => Duration::minutes(count),
        "h" => Duration::hours(count),
        "d" => Duration::days(count),
        "w" => Duration::weeks(count),
        "M" => Duration::days(count * 30),  // approximate month
        "y" => Duration::days(count * 365), // approximate year
        _ => {
            return Err(MermaidError::Layout(format!(
                "Unknown duration unit: '{}'",
                unit
            )))
        }
    };

    Ok(duration)
}

/// Extend an end date to skip excluded days for duration-based tasks.
fn apply_excludes_to_end(
    start: NaiveDateTime,
    end: NaiveDateTime,
    duration: Duration,
    exclusion_rules: &ExclusionRules,
) -> NaiveDateTime {
    if exclusion_rules.excludes.is_empty() {
        return end;
    }

    // Count working days needed
    let total_days = duration.num_days();
    if total_days <= 0 {
        return end;
    }

    let mut working_days = 0i64;
    let mut current = start;

    loop {
        if working_days >= total_days {
            break;
        }
        current += Duration::days(1);
        if !is_excluded(current.date(), exclusion_rules) {
            working_days += 1;
        }
    }

    current
}

/// If a start date falls on an excluded day, advance to the next non-excluded day.
fn skip_excluded_start(start: NaiveDateTime, exclusion_rules: &ExclusionRules) -> NaiveDateTime {
    if exclusion_rules.excludes.is_empty() {
        return start;
    }
    let mut current = start;
    // Safety limit to avoid infinite loop
    for _ in 0..30 {
        if !is_excluded(current.date(), exclusion_rules) {
            return current;
        }
        current += Duration::days(1);
    }
    current
}

/// Check if a date is excluded by the exclude rules.
fn is_excluded(date: NaiveDate, exclusion_rules: &ExclusionRules) -> bool {
    // Check includes first — they override excludes
    for inc in &exclusion_rules.includes {
        if rule_matches(date, inc) {
            return false;
        }
    }

    // Check excludes
    for exc in &exclusion_rules.excludes {
        if rule_matches(date, exc) {
            return true;
        }
    }

    false
}

fn rule_matches(date: NaiveDate, rule: &DateRule) -> bool {
    match rule {
        DateRule::Weekends => {
            let wd = date.weekday();
            wd == Weekday::Sat || wd == Weekday::Sun
        }
        DateRule::Weekday(day) => date.weekday() == *day,
        DateRule::ExactDate(exact) => date == *exact,
    }
}

/// Parse and normalize exclude/include rules once to avoid per-day string work.
fn compile_exclusion_rules(excludes: &[String], includes: &[String]) -> ExclusionRules {
    ExclusionRules {
        includes: includes.iter().filter_map(|s| parse_date_rule(s)).collect(),
        excludes: excludes.iter().filter_map(|s| parse_date_rule(s)).collect(),
    }
}

fn parse_date_rule(raw: &str) -> Option<DateRule> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }
    if value == "weekends" {
        return Some(DateRule::Weekends);
    }
    if let Some(day) = parse_weekday(&value) {
        return Some(DateRule::Weekday(day));
    }
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .ok()
        .map(DateRule::ExactDate)
}

fn parse_weekday(day_name: &str) -> Option<Weekday> {
    match day_name {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

// ── Grid line generation ────────────────────────────────────

fn generate_grid_lines(
    ast: &GanttAst,
    time_start: NaiveDateTime,
    time_end: NaiveDateTime,
    time_span: f64,
    chart_x: f64,
    chart_width: f64,
    measurer: &TextMeasurer,
) -> Vec<GridLine> {
    let mut lines = Vec::new();
    let fmt = axis_format_str(ast);

    // Determine tick interval
    let total_days = (time_end - time_start).num_days();

    let (tick_days, _) = if let Some(ref ti) = ast.tick_interval {
        match ti.unit {
            TickUnit::Day => (ti.count as i64, "day"),
            TickUnit::Week => (ti.count as i64 * 7, "week"),
            TickUnit::Month => (ti.count as i64 * 30, "month"),
        }
    } else {
        // Auto-determine tick interval based on range
        if total_days <= 14 {
            (1, "day")
        } else if total_days <= 60 {
            (7, "week")
        } else if total_days <= 365 {
            (30, "month")
        } else {
            (90, "quarter")
        }
    };

    let mut current = time_start;
    while current <= time_end {
        let offset = (current - time_start).num_seconds() as f64;
        let x = chart_x + (offset / time_span) * chart_width;

        let label = current.format(&fmt).to_string();
        lines.push(GridLine {
            x,
            label,
            show_label: true, // will be refined below
        });

        current += Duration::days(tick_days);
    }

    // Hide labels that would overlap — measure each label and check spacing
    let label_padding = 8.0; // minimum px gap between labels
    let mut last_label_right: f64 = f64::NEG_INFINITY;

    for line in &mut lines {
        let lw = measurer.measure(&line.label).width;
        let label_left = line.x - lw / 2.0;
        if label_left < last_label_right + label_padding {
            line.show_label = false;
        } else {
            line.show_label = true;
            last_label_right = line.x + lw / 2.0;
        }
    }

    lines
}

fn axis_format_str(ast: &GanttAst) -> String {
    ast.axis_format
        .clone()
        .unwrap_or_else(|| "%Y-%m-%d".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;

    fn make_measurer() -> (FontProvider, Theme) {
        (FontProvider::default_font(), Theme::default())
    }

    #[test]
    fn test_dayjs_to_chrono_format() {
        assert_eq!(dayjs_to_chrono_format("YYYY-MM-DD"), "%Y-%m-%d");
        assert_eq!(dayjs_to_chrono_format("DD/MM/YYYY"), "%d/%m/%Y");
        assert_eq!(dayjs_to_chrono_format("YYYY-MM-DD HH:mm"), "%Y-%m-%d %H:%M");
    }

    #[test]
    fn test_parse_date() {
        let dt = parse_date("2014-01-01", "%Y-%m-%d").unwrap();
        assert_eq!(dt.date(), NaiveDate::from_ymd_opt(2014, 1, 1).unwrap());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("3d").unwrap(), Duration::days(3));
        assert_eq!(parse_duration("24h").unwrap(), Duration::hours(24));
        assert_eq!(parse_duration("2w").unwrap(), Duration::weeks(2));
        assert_eq!(parse_duration("1M").unwrap(), Duration::days(30));
    }

    #[test]
    fn test_is_excluded_weekends() {
        let sat = NaiveDate::from_ymd_opt(2014, 1, 4).unwrap(); // Saturday
        let sun = NaiveDate::from_ymd_opt(2014, 1, 5).unwrap(); // Sunday
        let mon = NaiveDate::from_ymd_opt(2014, 1, 6).unwrap(); // Monday

        let excludes = vec!["weekends".to_string()];
        let includes: Vec<String> = vec![];
        let rules = compile_exclusion_rules(&excludes, &includes);

        assert!(is_excluded(sat, &rules));
        assert!(is_excluded(sun, &rules));
        assert!(!is_excluded(mon, &rules));
    }

    #[test]
    fn test_is_excluded_day_name() {
        let mon = NaiveDate::from_ymd_opt(2014, 1, 6).unwrap(); // Monday
        let tue = NaiveDate::from_ymd_opt(2014, 1, 7).unwrap(); // Tuesday

        let excludes = vec!["monday".to_string()];
        let includes: Vec<String> = vec![];
        let rules = compile_exclusion_rules(&excludes, &includes);

        assert!(is_excluded(mon, &rules));
        assert!(!is_excluded(tue, &rules));
    }

    #[test]
    fn test_includes_override_excludes() {
        let sat = NaiveDate::from_ymd_opt(2014, 1, 4).unwrap(); // Saturday

        let excludes = vec!["weekends".to_string()];
        let includes = vec!["saturday".to_string()];
        let rules = compile_exclusion_rules(&excludes, &includes);

        assert!(!is_excluded(sat, &rules));
    }

    #[test]
    fn test_layout_simple_gantt() {
        let ast = GanttAst {
            title: Some("Test Gantt".to_string()),
            date_format: "YYYY-MM-DD".to_string(),
            sections: vec![GanttSection {
                name: "Section A".to_string(),
                tasks: vec![
                    GanttTask {
                        name: "Task 1".to_string(),
                        tags: TaskTags::default(),
                        id: Some("t1".to_string()),
                        depends_on: Vec::new(),
                        start: TaskStart::Date("2014-01-01".to_string()),
                        end: TaskEnd::Duration("3d".to_string()),
                    },
                    GanttTask {
                        name: "Task 2".to_string(),
                        tags: TaskTags::default(),
                        id: Some("t2".to_string()),
                        depends_on: Vec::new(),
                        start: TaskStart::After(vec!["t1".to_string()]),
                        end: TaskEnd::Duration("5d".to_string()),
                    },
                ],
            }],
            ..Default::default()
        };

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        assert_eq!(layout.tasks.len(), 2);
        assert_eq!(layout.sections.len(), 1);

        // Task 2 should start after Task 1
        assert!(layout.tasks[1].x >= layout.tasks[0].x + layout.tasks[0].width - 1.0);
    }

    #[test]
    fn test_layout_empty_gantt() {
        let ast = GanttAst::default();

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        assert!(layout.tasks.is_empty());
    }

    #[test]
    fn test_layout_with_excludes() {
        let ast = GanttAst {
            date_format: "YYYY-MM-DD".to_string(),
            excludes: vec!["weekends".to_string()],
            sections: vec![GanttSection {
                name: "Test".to_string(),
                tasks: vec![GanttTask {
                    name: "Task".to_string(),
                    tags: TaskTags::default(),
                    id: Some("t1".to_string()),
                    depends_on: Vec::new(),
                    start: TaskStart::Date("2014-01-06".to_string()), // Monday
                    end: TaskEnd::Duration("5d".to_string()),
                }],
            }],
            ..Default::default()
        };

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.tasks.len(), 1);
        // With weekend excludes, a 5-day task starting Monday should end on the following Monday
        // (skipping Saturday and Sunday)
        assert!(layout.tasks[0].width > 0.0);
    }

    #[test]
    fn test_layout_resolves_dependency_edges() {
        let ast = GanttAst {
            date_format: "YYYY-MM-DD".to_string(),
            sections: vec![GanttSection {
                name: "Deps".to_string(),
                tasks: vec![
                    GanttTask {
                        name: "Task A".to_string(),
                        tags: TaskTags::default(),
                        id: Some("a1".to_string()),
                        depends_on: Vec::new(),
                        start: TaskStart::Date("2014-01-01".to_string()),
                        end: TaskEnd::Duration("3d".to_string()),
                    },
                    GanttTask {
                        name: "Task B".to_string(),
                        tags: TaskTags::default(),
                        id: Some("b1".to_string()),
                        depends_on: vec!["a1".to_string(), "missing".to_string()],
                        start: TaskStart::Date("2014-01-05".to_string()),
                        end: TaskEnd::Duration("2d".to_string()),
                    },
                ],
            }],
            ..Default::default()
        };

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.tasks.len(), 2);
        assert_eq!(layout.dependency_edges.len(), 1);
        assert_eq!(layout.dependency_edges[0].from_task_index, 0);
        assert_eq!(layout.dependency_edges[0].to_task_index, 1);
    }

    #[test]
    fn test_layout_dedupes_transitive_dependency_edges() {
        let ast = GanttAst {
            date_format: "YYYY-MM-DD".to_string(),
            sections: vec![GanttSection {
                name: "Deps".to_string(),
                tasks: vec![
                    GanttTask {
                        name: "Task A".to_string(),
                        tags: TaskTags::default(),
                        id: Some("a1".to_string()),
                        depends_on: Vec::new(),
                        start: TaskStart::Date("2014-01-01".to_string()),
                        end: TaskEnd::Duration("2d".to_string()),
                    },
                    GanttTask {
                        name: "Task B".to_string(),
                        tags: TaskTags::default(),
                        id: Some("b1".to_string()),
                        depends_on: vec!["a1".to_string()],
                        start: TaskStart::Date("2014-01-03".to_string()),
                        end: TaskEnd::Duration("2d".to_string()),
                    },
                    GanttTask {
                        name: "Task C".to_string(),
                        tags: TaskTags::default(),
                        id: Some("c1".to_string()),
                        depends_on: vec!["a1".to_string(), "b1".to_string()],
                        start: TaskStart::Date("2014-01-05".to_string()),
                        end: TaskEnd::Duration("2d".to_string()),
                    },
                ],
            }],
            ..Default::default()
        };

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.dependency_edges.len(), 2);
        assert!(layout
            .dependency_edges
            .iter()
            .any(|e| e.from_task_index == 0 && e.to_task_index == 1));
        assert!(layout
            .dependency_edges
            .iter()
            .any(|e| e.from_task_index == 1 && e.to_task_index == 2));
        assert!(!layout
            .dependency_edges
            .iter()
            .any(|e| e.from_task_index == 0 && e.to_task_index == 2));
    }

    #[test]
    fn test_layout_expands_width_for_outside_labels() {
        let ast = GanttAst {
            title: Some("Label Width Test".to_string()),
            date_format: "YYYY-MM-DD".to_string(),
            sections: vec![GanttSection {
                name: "Section".to_string(),
                tasks: vec![GanttTask {
                    name: "Final milestone label should not overflow chart bounds".to_string(),
                    tags: TaskTags {
                        milestone: true,
                        ..Default::default()
                    },
                    id: Some("m1".to_string()),
                    depends_on: Vec::new(),
                    start: TaskStart::Date("2014-01-10".to_string()),
                    end: TaskEnd::Duration("0d".to_string()),
                }],
            }],
            ..Default::default()
        };

        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gantt(&ast, &measurer, &theme).unwrap();

        let base_width = layout.chart_x + layout.chart_width + RIGHT_PADDING;
        assert!(layout.width > base_width);
    }
}
