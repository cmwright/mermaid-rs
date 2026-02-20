use crate::ast::gitgraph::CommitType;
use crate::error::Result;
use crate::layout::gitgraph::*;
use crate::render::svg_util::{build_orthogonal_path, escape_xml};
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;
const COMMIT_RADIUS: f64 = 10.0;

// Branch color palette
const BRANCH_COLORS: &[&str] = &[
    "#0049B7", // main - blue
    "#B7A400", // branch 1 - yellow-green
    "#00B746", // branch 2 - green
    "#B74600", // branch 3 - orange
    "#5500B7", // branch 4 - purple
    "#B70049", // branch 5 - pink
    "#00B7B7", // branch 6 - cyan
    "#B75500", // branch 7 - dark orange
];

pub fn render_svg(layout: &GitGraphLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    let mut svg = String::with_capacity(8192);

    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    ));
    svg.push('\n');

    // Style block
    svg.push_str(&format!(
        "<style>\
\n  svg {{ background: {}; }}\
\n  .git-label {{ font-family: {}; font-size: {:.0}px; font-weight: bold; }}\
\n  .git-hash {{ font-family: {}; font-size: {:.0}px; }}\
\n  .git-tag {{ font-family: {}; font-size: {:.0}px; }}\
\n</style>",
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.75,
        theme.font_family,
        theme.font_size * 0.85,
    ));
    svg.push('\n');

    // Content group
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // Layer 1: Branch lines
    for line in &layout.branch_lines {
        svg.push_str(&render_branch_line(line));
    }

    // Layer 2: Connections (merge/branch curves)
    for conn in &layout.connections {
        svg.push_str(&render_connection(conn));
    }

    // Layer 3: Commit nodes
    for commit in &layout.commits {
        svg.push_str(&render_commit(commit));
    }

    // Layer 4: Tags
    for tag in &layout.tags {
        svg.push_str(&render_tag(tag));
    }

    // Layer 5: Commit hash labels
    for commit in &layout.commits {
        svg.push_str(&render_commit_label(commit));
    }

    // Layer 6: Branch labels
    for label in &layout.branch_labels {
        svg.push_str(&render_branch_label(label));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn get_color(index: usize) -> &'static str {
    BRANCH_COLORS[index % BRANCH_COLORS.len()]
}

fn render_branch_line(line: &PositionedBranchLine) -> String {
    let color = get_color(line.color_index);
    if line.is_dotted {
        format!(
            "  <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"2\" stroke-dasharray=\"5,5\" opacity=\"0.4\"/>\n",
            line.x_start, line.y, line.x_end, line.y, color,
        )
    } else {
        format!(
            r#"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="3"/>"#,
            line.x_start, line.y, line.x_end, line.y, color,
        ) + "\n"
    }
}

fn render_connection(conn: &PositionedConnection) -> String {
    let color = get_color(conn.color_index);
    let path_d = build_orthogonal_path(&conn.points);
    format!(
        r#"  <path d="{}" fill="none" stroke="{}" stroke-width="3"/>"#,
        path_d, color,
    ) + "\n"
}

fn render_commit(commit: &PositionedCommit) -> String {
    let color = get_color(commit.color_index);

    // Merge commits: hollow circle (white fill, colored stroke)
    if commit.is_merge {
        return format!(
            "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\" fill=\"#ffffff\" stroke=\"{}\" stroke-width=\"3\"/>\n",
            commit.x, commit.y, COMMIT_RADIUS, color,
        );
    }

    match commit.commit_type {
        CommitType::Normal => {
            format!(
                r#"  <circle cx="{:.1}" cy="{:.1}" r="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                commit.x, commit.y, COMMIT_RADIUS, color, color,
            ) + "\n"
        }
        CommitType::Highlight => {
            let half = COMMIT_RADIUS;
            let white = "#ffffff";
            format!(
                "  <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"3\"/>\n",
                commit.x - half,
                commit.y - half,
                half * 2.0,
                half * 2.0,
                white,
                color,
            )
        }
        CommitType::Reverse => {
            let white = "#ffffff";
            let mut s = format!(
                r#"  <circle cx="{:.1}" cy="{:.1}" r="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                commit.x, commit.y, COMMIT_RADIUS, color, color,
            );
            s.push('\n');
            s.push_str(&format!(
                "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                commit.x,
                commit.y,
                COMMIT_RADIUS - 3.0,
                white,
            ));
            s
        }
    }
}

fn render_tag(tag: &PositionedTag) -> String {
    let mut s = String::new();
    let h = 20.0;
    let arrow_w = 8.0;
    let x = tag.x - tag.width / 2.0;
    let y = tag.y;
    let tag_fill = "#ffffcc";
    let tag_stroke = "#aaaa33";

    // Tag shape: rectangle with pointed left side
    s.push_str(&format!(
        "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
        x - arrow_w, y + h / 2.0,     // left point
        x, y,                          // top-left
        x + tag.width, y,              // top-right
        x + tag.width, y + h,          // bottom-right
        x, y + h,                      // bottom-left
        tag_fill, tag_stroke,
    ));

    // Small circle at the tag point
    s.push_str(&format!(
        "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\" fill=\"{}\"/>\n",
        x - arrow_w,
        y + h / 2.0,
        tag_stroke,
    ));

    // Tag text
    s.push_str(&format!(
        "  <text class=\"git-tag\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>\n",
        x + tag.width / 2.0,
        y + h / 2.0,
        escape_xml(&tag.text),
    ));

    s
}

fn render_commit_label(commit: &PositionedCommit) -> String {
    let color = get_color(commit.color_index);
    format!(
        "  <text class=\"git-hash\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"hanging\" fill=\"{}\">{}</text>\n",
        commit.x,
        commit.y + COMMIT_RADIUS + 4.0,
        color,
        escape_xml(&commit.id),
    )
}

fn render_branch_label(label: &PositionedBranchLabel) -> String {
    let color = get_color(label.color_index);
    let mut s = String::new();

    let rect_h = 24.0;
    let rx = rect_h / 2.0;

    // Rounded rectangle background
    s.push_str(&format!(
        r#"  <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{}" rx="{}" fill="{}" stroke="none"/>"#,
        label.x,
        label.y - rect_h / 2.0,
        label.width,
        rect_h,
        rx,
        color,
    ));
    s.push('\n');

    // Branch name text (white on colored background)
    let white = "#ffffff";
    s.push_str(&format!(
        "  <text class=\"git-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\">{}</text>\n",
        label.x + label.width / 2.0,
        label.y,
        white,
        escape_xml(&label.name),
    ));

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::theme::Theme;

    fn minimal_layout(branch_lines: Vec<PositionedBranchLine>) -> GitGraphLayout {
        GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines,
            commits: vec![],
            connections: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn test_dotted_branch_line_contains_stroke_dasharray() {
        let layout = minimal_layout(vec![PositionedBranchLine {
            y: 50.0,
            x_start: 10.0,
            x_end: 200.0,
            color_index: 0,
            is_dotted: true,
        }]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("stroke-dasharray"),
            "Dotted branch line SVG should contain stroke-dasharray"
        );
    }

    #[test]
    fn test_solid_branch_line_no_stroke_dasharray() {
        let layout = minimal_layout(vec![PositionedBranchLine {
            y: 50.0,
            x_start: 10.0,
            x_end: 200.0,
            color_index: 0,
            is_dotted: false,
        }]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            !svg.contains("stroke-dasharray"),
            "Solid branch line SVG should not contain stroke-dasharray"
        );
    }
}
