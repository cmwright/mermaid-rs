/// Top-level AST for a Gantt chart.
#[derive(Debug, Clone)]
pub struct GanttAst {
    pub title: Option<String>,
    /// Date format string (dayjs-style, e.g. "YYYY-MM-DD")
    pub date_format: String,
    /// Axis format string (strftime-style, e.g. "%Y-%m-%d")
    pub axis_format: Option<String>,
    pub tick_interval: Option<TickInterval>,
    pub inclusive_end_dates: bool,
    pub today_marker: TodayMarker,
    /// Excluded days: "weekends", day names ("monday"), specific dates
    pub excludes: Vec<String>,
    /// Included days that override excludes
    pub includes: Vec<String>,
    pub sections: Vec<GanttSection>,
}

impl Default for GanttAst {
    fn default() -> Self {
        Self {
            title: None,
            date_format: "YYYY-MM-DD".to_string(),
            axis_format: None,
            tick_interval: None,
            inclusive_end_dates: false,
            today_marker: TodayMarker::On,
            excludes: Vec::new(),
            includes: Vec::new(),
            sections: Vec::new(),
        }
    }
}

/// A section grouping tasks in the Gantt chart.
#[derive(Debug, Clone)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

/// A single task in the Gantt chart.
#[derive(Debug, Clone)]
pub struct GanttTask {
    pub name: String,
    pub tags: TaskTags,
    /// Explicit ID or None for auto-generated
    pub id: Option<String>,
    pub start: TaskStart,
    pub end: TaskEnd,
}

/// Tags that modify a task's appearance/state.
#[derive(Debug, Clone, Default)]
pub struct TaskTags {
    pub done: bool,
    pub active: bool,
    pub crit: bool,
    pub milestone: bool,
}

/// How a task's start time is specified.
#[derive(Debug, Clone)]
pub enum TaskStart {
    /// An explicit date string (parsed with dateFormat)
    Date(String),
    /// After one or more other tasks complete
    After(Vec<String>),
    /// Starts when the previous task ends (implicit)
    PrevEnd,
}

/// How a task's end time is specified.
#[derive(Debug, Clone)]
pub enum TaskEnd {
    /// An explicit date string (parsed with dateFormat)
    Date(String),
    /// A duration string (e.g. "3d", "24h", "2w")
    Duration(String),
    /// Until one or more other tasks start
    Until(Vec<String>),
}

/// Whether the today marker is shown.
#[derive(Debug, Clone, PartialEq)]
pub enum TodayMarker {
    On,
    Off,
}

/// Tick interval for the time axis.
#[derive(Debug, Clone)]
pub struct TickInterval {
    pub count: u32,
    pub unit: TickUnit,
}

/// Units for tick intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickUnit {
    Day,
    Week,
    Month,
}
