use crate::ast::pie::*;
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;
use crate::render::theme::Theme;

// ── Constants ────────────────────────────────────────────────

const PIE_RADIUS: f64 = 150.0;
const PIE_CENTER_X: f64 = 200.0;
const PIE_CENTER_Y: f64 = 200.0;
const TITLE_HEIGHT: f64 = 40.0;
const TITLE_PADDING: f64 = 20.0;
const LEGEND_ITEM_HEIGHT: f64 = 25.0;
const _LEGEND_BOX_SIZE: f64 = 16.0;
const LEGEND_PADDING: f64 = 30.0;
const LEGEND_TEXT_OFFSET: f64 = 24.0;
const DIAGRAM_PADDING: f64 = 30.0;
const LABEL_RADIUS_FACTOR: f64 = 0.7; // Place labels at 70% of radius

// ── Positioned output types ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct PieLayout {
    pub width: f64,
    pub height: f64,
    pub title: Option<String>,
    pub title_y: f64,
    pub pie_center_x: f64,
    pub pie_center_y: f64,
    pub pie_radius: f64,
    pub slices: Vec<PositionedSlice>,
    pub legend: Vec<LegendItem>,
    pub legend_x: f64,
    pub legend_y: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedSlice {
    pub label: String,
    pub value: f64,
    pub percentage: f64,
    pub start_angle: f64, // in radians
    pub end_angle: f64,   // in radians
    pub label_x: f64,
    pub label_y: f64,
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct LegendItem {
    pub label: String,
    pub color_index: usize,
    pub y: f64,
}

// ── Layout algorithm ────────────────────────────────────────

pub fn layout_pie(ast: &PieAst, measurer: &TextMeasurer, _theme: &Theme) -> Result<PieLayout> {
    // Calculate total value
    let total: f64 = ast.slices.iter().map(|s| s.value).sum();

    if total == 0.0 {
        // Handle empty pie chart
        return Ok(PieLayout {
            width: PIE_CENTER_X * 2.0,
            height: PIE_CENTER_Y * 2.0 + TITLE_HEIGHT,
            title: ast.title.clone(),
            title_y: TITLE_PADDING,
            pie_center_x: PIE_CENTER_X,
            pie_center_y: PIE_CENTER_Y + TITLE_HEIGHT,
            pie_radius: PIE_RADIUS,
            slices: Vec::new(),
            legend: Vec::new(),
            legend_x: 0.0,
            legend_y: 0.0,
        });
    }

    // Calculate slice angles and positions
    let mut slices = Vec::new();
    let mut current_angle: f64 = -std::f64::consts::PI / 2.0; // Start at top (12 o'clock)

    for (i, slice) in ast.slices.iter().enumerate() {
        let percentage = slice.value / total;
        let angle_span = percentage * 2.0 * std::f64::consts::PI;
        let end_angle = current_angle + angle_span;

        // Calculate label position (at the middle of the slice)
        let mid_angle = current_angle + angle_span / 2.0;
        let label_r = PIE_RADIUS * LABEL_RADIUS_FACTOR;
        let label_x = PIE_CENTER_X + label_r * mid_angle.cos();
        let label_y = PIE_CENTER_Y + TITLE_HEIGHT + label_r * mid_angle.sin();

        slices.push(PositionedSlice {
            label: slice.label.clone(),
            value: slice.value,
            percentage: percentage * 100.0,
            start_angle: current_angle,
            end_angle,
            label_x,
            label_y,
            color_index: i,
        });

        current_angle = end_angle;
    }

    // Calculate legend dimensions
    let legend_items: Vec<LegendItem> = slices
        .iter()
        .enumerate()
        .map(|(i, slice)| LegendItem {
            label: slice.label.clone(),
            color_index: slice.color_index,
            y: i as f64 * LEGEND_ITEM_HEIGHT,
        })
        .collect();

    // Calculate legend width based on longest label
    let max_label_width = slices
        .iter()
        .map(|s| measurer.measure(&s.label).width)
        .fold(0.0_f64, f64::max);
    let legend_width = LEGEND_TEXT_OFFSET + max_label_width + LEGEND_PADDING;

    // Position legend to the right of the pie
    let legend_x = PIE_CENTER_X + PIE_RADIUS + LEGEND_PADDING;
    let legend_y =
        PIE_CENTER_Y + TITLE_HEIGHT - (legend_items.len() as f64 * LEGEND_ITEM_HEIGHT) / 2.0;

    // Calculate total dimensions
    let width = legend_x + legend_width + DIAGRAM_PADDING;
    let height = (PIE_CENTER_Y + TITLE_HEIGHT + PIE_RADIUS + DIAGRAM_PADDING)
        .max(legend_y + legend_items.len() as f64 * LEGEND_ITEM_HEIGHT + DIAGRAM_PADDING);

    Ok(PieLayout {
        width,
        height,
        title: ast.title.clone(),
        title_y: TITLE_PADDING,
        pie_center_x: PIE_CENTER_X,
        pie_center_y: PIE_CENTER_Y + TITLE_HEIGHT,
        pie_radius: PIE_RADIUS,
        slices,
        legend: legend_items,
        legend_x,
        legend_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;

    fn make_measurer() -> (FontProvider, Theme) {
        (FontProvider::default_font(), Theme::default())
    }

    #[test]
    fn test_layout_simple_pie() {
        let ast = PieAst {
            title: Some("Test".to_string()),
            slices: vec![
                PieSlice {
                    label: "A".to_string(),
                    value: 50.0,
                },
                PieSlice {
                    label: "B".to_string(),
                    value: 50.0,
                },
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        assert_eq!(layout.slices.len(), 2);
        assert_eq!(layout.legend.len(), 2);

        // Each slice should be 50%
        assert!((layout.slices[0].percentage - 50.0).abs() < 0.01);
        assert!((layout.slices[1].percentage - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_layout_pie_percentages() {
        let ast = PieAst {
            title: None,
            slices: vec![
                PieSlice {
                    label: "A".to_string(),
                    value: 25.0,
                },
                PieSlice {
                    label: "B".to_string(),
                    value: 75.0,
                },
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert!((layout.slices[0].percentage - 25.0).abs() < 0.01);
        assert!((layout.slices[1].percentage - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_layout_empty_pie() {
        // Empty pie chart: no slices (total = 0)
        let ast = PieAst {
            title: None,
            slices: Vec::new(),
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert!(layout.slices.is_empty());
        assert!(layout.legend.is_empty());
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        assert_eq!(layout.pie_radius, PIE_RADIUS);
    }

    #[test]
    fn test_layout_empty_pie_zero_values() {
        // Pie chart with slices that all have zero value
        let ast = PieAst {
            title: Some("Empty Data".to_string()),
            slices: vec![
                PieSlice {
                    label: "A".to_string(),
                    value: 0.0,
                },
                PieSlice {
                    label: "B".to_string(),
                    value: 0.0,
                },
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        // Total is 0, so should return empty slices layout
        assert!(layout.slices.is_empty());
        assert_eq!(layout.title.as_deref(), Some("Empty Data"));
    }

    #[test]
    fn test_layout_pie_with_title() {
        let ast = PieAst {
            title: Some("My Pie Chart".to_string()),
            slices: vec![
                PieSlice {
                    label: "X".to_string(),
                    value: 30.0,
                },
                PieSlice {
                    label: "Y".to_string(),
                    value: 70.0,
                },
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.title.as_deref(), Some("My Pie Chart"));
        assert_eq!(layout.title_y, TITLE_PADDING);
        // Pie center should be shifted down by TITLE_HEIGHT
        assert!((layout.pie_center_y - (PIE_CENTER_Y + TITLE_HEIGHT)).abs() < 0.01);
    }

    #[test]
    fn test_layout_pie_no_title() {
        let ast = PieAst {
            title: None,
            slices: vec![PieSlice {
                label: "Only".to_string(),
                value: 100.0,
            }],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert!(layout.title.is_none());
        assert_eq!(layout.slices.len(), 1);
        assert!((layout.slices[0].percentage - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_layout_pie_legend_positioning() {
        let ast = PieAst {
            title: Some("Legend Test".to_string()),
            slices: vec![
                PieSlice {
                    label: "Alpha".to_string(),
                    value: 10.0,
                },
                PieSlice {
                    label: "Beta".to_string(),
                    value: 20.0,
                },
                PieSlice {
                    label: "Gamma".to_string(),
                    value: 30.0,
                },
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_pie(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.legend.len(), 3);
        // Legend should be to the right of the pie
        assert!(layout.legend_x > layout.pie_center_x + layout.pie_radius);
        // Legend items should be spaced vertically
        assert!((layout.legend[1].y - layout.legend[0].y - LEGEND_ITEM_HEIGHT).abs() < 0.01);
    }
}
