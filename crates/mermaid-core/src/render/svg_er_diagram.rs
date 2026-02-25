use std::fmt::Write;

use crate::ast::er_diagram::{AttributeKey, Cardinality, RelationType};
use crate::error::Result;
use crate::layout::er_diagram::types::*;
use crate::render::svg_util::{build_basis_curve_path, escape_xml};
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;
const ROW_HEIGHT: f64 = 24.0;

// Cardinality marker dimensions
const MARKER_LINE_HALF: f64 = 3.5;
const MARKER_CIRCLE_R: f64 = 2.5;
const CROW_SPREAD: f64 = 3.5;

/// Render a positioned ER diagram to an SVG string.
pub fn render_svg(diagram: &PositionedErDiagram, theme: &Theme) -> Result<String> {
    let view_w = diagram.width + 2.0 * SVG_PADDING;
    let view_h = diagram.height + 2.0 * SVG_PADDING;

    let est_capacity = 2048 + diagram.entities.len() * 500 + diagram.relationships.len() * 400;
    let mut svg = String::with_capacity(est_capacity);

    // SVG header
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    );
    svg.push('\n');

    // Style block
    let _ = write!(
        svg,
        r#"<style>
  svg {{ background: {}; }}
  .er-entity-text {{ font-family: {}; font-size: {}px; }}
  .er-attr-text {{ font-family: {}; font-size: {}px; }}
  .er-edge-label {{ font-family: {}; font-size: {}px; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.9,
        theme.font_family,
        theme.font_size * 0.85,
    );
    svg.push('\n');

    // Content group with padding offset
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    );
    svg.push('\n');

    // Render relationships first (behind entities)
    for rel in &diagram.relationships {
        render_relationship(&mut svg, rel, theme);
    }

    // Render entities on top
    for entity in &diagram.entities {
        render_entity(&mut svg, entity, theme);
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn render_entity(svg: &mut String, entity: &PositionedEntity, theme: &Theme) {
    let er = &theme.er_diagram;
    let hw = entity.width / 2.0;
    let hh = entity.height / 2.0;

    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        entity.x, entity.y
    );
    svg.push('\n');

    // Outer rectangle
    let _ = write!(
        svg,
        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5"/>"#,
        -hw,
        -hh,
        entity.width,
        entity.height,
        er.entity_fill.to_css(),
        er.entity_border.to_css(),
    );
    svg.push('\n');

    // Header background (inset by half stroke width to stay inside the border)
    let inset = 0.75; // half of stroke-width 1.5
    let inner_x = -hw + inset;
    let inner_w = entity.width - 2.0 * inset;
    let _ = write!(
        svg,
        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
        inner_x,
        -hh + inset,
        inner_w,
        entity.header_height - inset,
        er.entity_header_fill.to_css(),
    );
    svg.push('\n');

    // Header text (entity name, centered)
    let _ = write!(
        svg,
        r#"  <text class="er-entity-text" x="0" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
        -hh + entity.header_height / 2.0,
        er.entity_text.to_css(),
        escape_xml(&entity.id),
    );
    svg.push('\n');

    // Divider line after header
    if !entity.attributes.is_empty() {
        let divider_y = -hh + entity.header_height;
        let _ = write!(
            svg,
            r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
            -hw,
            divider_y,
            hw,
            divider_y,
            er.entity_border.to_css(),
        );
        svg.push('\n');

        // Attribute rows
        let body_top = -hh + entity.header_height + 1.0; // after divider
        for (i, attr) in entity.attributes.iter().enumerate() {
            let row_y = body_top + i as f64 * ROW_HEIGHT;

            // Alternating row background (inset to stay inside border, clamped to entity bottom)
            if i % 2 == 1 {
                let row_h = ROW_HEIGHT.min(hh - inset - row_y);
                if row_h > 0.0 {
                    let _ = write!(
                        svg,
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                        inner_x,
                        row_y,
                        inner_w,
                        row_h,
                        er.attr_row_alt_fill.to_css(),
                    );
                    svg.push('\n');
                }
            }

            // Compute column x positions
            let padding_h = 12.0;
            let col_gap = 12.0;
            let mut col_x = -hw + padding_h;
            let text_y = row_y + ROW_HEIGHT / 2.0;

            // Type column
            let _ = write!(
                svg,
                r#"  <text class="er-attr-text" x="{}" y="{}" dominant-baseline="central" fill="{}">{}</text>"#,
                col_x,
                text_y,
                er.entity_text.to_css(),
                escape_xml(&attr.type_name),
            );
            svg.push('\n');
            col_x += entity.col_widths[0] + col_gap;

            // Name column
            let _ = write!(
                svg,
                r#"  <text class="er-attr-text" x="{}" y="{}" dominant-baseline="central" font-weight="bold" fill="{}">{}</text>"#,
                col_x,
                text_y,
                er.entity_text.to_css(),
                escape_xml(&attr.name),
            );
            svg.push('\n');
            col_x += entity.col_widths[1] + col_gap;

            // Key column
            if attr.key != AttributeKey::None {
                let key_str = match attr.key {
                    AttributeKey::PK => "PK",
                    AttributeKey::FK => "FK",
                    AttributeKey::UK => "UK",
                    AttributeKey::None => unreachable!(),
                };
                let _ = write!(
                    svg,
                    r#"  <text class="er-attr-text" x="{}" y="{}" dominant-baseline="central" fill="{}" font-style="italic">{}</text>"#,
                    col_x,
                    text_y,
                    er.entity_text.to_css(),
                    key_str,
                );
                svg.push('\n');
            }
            col_x += entity.col_widths[2] + col_gap;

            // Comment column
            if let Some(comment) = &attr.comment {
                let _ = write!(
                    svg,
                    r#"  <text class="er-attr-text" x="{}" y="{}" dominant-baseline="central" fill="{}" opacity="0.7">{}</text>"#,
                    col_x,
                    text_y,
                    er.entity_text.to_css(),
                    escape_xml(comment),
                );
                svg.push('\n');
            }
        }
    }

    svg.push_str("</g>\n");
}

fn render_relationship(svg: &mut String, rel: &PositionedRelationship, theme: &Theme) {
    if rel.points.len() < 2 {
        return;
    }

    let line_color = theme.line_color.to_css();
    let bg_color = theme.background.to_css();
    let stroke_width = theme.er_diagram.edge_width.max(1.0);

    let dash_attr = match rel.relation_type {
        RelationType::NonIdentifying => r#" stroke-dasharray="6,4""#,
        RelationType::Identifying => "",
    };

    // Draw the main path line (no markers)
    let path_d = build_basis_curve_path(&rel.points);
    let _ = write!(
        svg,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}"{} stroke-linecap="round" stroke-linejoin="round"/>"#,
        path_d, line_color, stroke_width, dash_attr,
    );
    svg.push('\n');

    // Draw cardinality symbols at the start (entity A side)
    {
        let (ex, ey) = rel.points[0];
        let (p1x, p1y) = rel.points[1];
        let ddx = p1x - ex;
        let ddy = p1y - ey;
        let len = (ddx * ddx + ddy * ddy).sqrt();
        let (dx, dy) = if len > 0.001 {
            (ddx / len, ddy / len)
        } else {
            (0.0, 1.0)
        };
        draw_cardinality_symbols(
            svg,
            rel.cardinality_from,
            ex,
            ey,
            dx,
            dy,
            &line_color,
            &bg_color,
            stroke_width,
        );
    }

    // Draw cardinality symbols at the end (entity B side)
    {
        let n = rel.points.len();
        let (ex, ey) = rel.points[n - 1];
        let (p1x, p1y) = rel.points[n - 2];
        let ddx = p1x - ex;
        let ddy = p1y - ey;
        let len = (ddx * ddx + ddy * ddy).sqrt();
        let (dx, dy) = if len > 0.001 {
            (ddx / len, ddy / len)
        } else {
            (0.0, -1.0)
        };
        draw_cardinality_symbols(
            svg,
            rel.cardinality_to,
            ex,
            ey,
            dx,
            dy,
            &line_color,
            &bg_color,
            stroke_width,
        );
    }

    // Relationship label at midpoint
    if let (Some(label), Some(lx), Some(ly)) = (&rel.label, rel.label_x, rel.label_y) {
        let label_w = rel.label_width.unwrap_or(label.len() as f64 * 8.0 + 10.0);
        let label_h = rel.label_height.unwrap_or(20.0);
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="rgba(232,232,232,0.8)"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
        );
        svg.push('\n');

        let _ = write!(
            svg,
            r#"<text class="er-edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            lx,
            ly,
            theme.text_color.to_css(),
            escape_xml(label),
        );
        svg.push('\n');
    }
}

/// Draw inline cardinality symbols at a path endpoint.
///
/// - `(ex, ey)`: the point on the entity boundary
/// - `(dx, dy)`: unit vector pointing AWAY from the entity along the path
///
/// Symbols are drawn at increasing offsets from the entity edge:
/// - Inner symbol (closer to entity) = max constraint (ONE `|` or MANY `{`)
/// - Outer symbol (further from entity) = min constraint (ONE `|` or ZERO `o`)
fn draw_cardinality_symbols(
    svg: &mut String,
    cardinality: Cardinality,
    ex: f64,
    ey: f64,
    dx: f64,
    dy: f64,
    color: &str,
    bg_color: &str,
    stroke_width: f64,
) {
    // Perpendicular direction
    let nx = -dy;
    let ny = dx;

    match cardinality {
        Cardinality::OnlyOne => {
            // || : two perpendicular lines
            draw_perp_line(svg, ex, ey, dx, dy, nx, ny, 4.0, color, stroke_width);
            draw_perp_line(svg, ex, ey, dx, dy, nx, ny, 8.0, color, stroke_width);
        }
        Cardinality::ZeroOrOne => {
            // o| : perpendicular line + circle
            draw_perp_line(svg, ex, ey, dx, dy, nx, ny, 4.0, color, stroke_width);
            draw_zero_circle(svg, ex, ey, dx, dy, 10.0, color, bg_color, stroke_width);
        }
        Cardinality::OneOrMore => {
            // }| : crow's foot + perpendicular line
            draw_crows_foot(svg, ex, ey, dx, dy, nx, ny, 1.0, 7.0, color, stroke_width);
            draw_perp_line(svg, ex, ey, dx, dy, nx, ny, 9.0, color, stroke_width);
        }
        Cardinality::ZeroOrMore => {
            // }o : crow's foot + circle
            draw_crows_foot(svg, ex, ey, dx, dy, nx, ny, 1.0, 7.0, color, stroke_width);
            draw_zero_circle(svg, ex, ey, dx, dy, 11.0, color, bg_color, stroke_width);
        }
    }
}

/// Draw a perpendicular line crossing the path at the given offset from the entity edge.
fn draw_perp_line(
    svg: &mut String,
    ex: f64,
    ey: f64,
    dx: f64,
    dy: f64,
    nx: f64,
    ny: f64,
    offset: f64,
    color: &str,
    stroke_width: f64,
) {
    let cx = ex + offset * dx;
    let cy = ey + offset * dy;
    let x1 = cx - MARKER_LINE_HALF * nx;
    let y1 = cy - MARKER_LINE_HALF * ny;
    let x2 = cx + MARKER_LINE_HALF * nx;
    let y2 = cy + MARKER_LINE_HALF * ny;
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        x1, y1, x2, y2, color, stroke_width,
    );
    svg.push('\n');
}

/// Draw a "zero" circle on the path at the given offset from the entity edge.
/// Filled with background color to mask the line underneath.
fn draw_zero_circle(
    svg: &mut String,
    ex: f64,
    ey: f64,
    dx: f64,
    dy: f64,
    offset: f64,
    color: &str,
    bg_color: &str,
    stroke_width: f64,
) {
    let cx = ex + offset * dx;
    let cy = ey + offset * dy;
    let _ = write!(
        svg,
        r#"<circle cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
        cx, cy, MARKER_CIRCLE_R, bg_color, color, stroke_width,
    );
    svg.push('\n');
}

/// Draw a crow's foot (two lines forming a V) pointing toward the entity.
/// `tip_offset`: how far from entity edge the tips of the V are
/// `base_offset`: how far from entity edge the convergence point is
#[allow(clippy::too_many_arguments)]
fn draw_crows_foot(
    svg: &mut String,
    ex: f64,
    ey: f64,
    dx: f64,
    dy: f64,
    nx: f64,
    ny: f64,
    tip_offset: f64,
    base_offset: f64,
    color: &str,
    stroke_width: f64,
) {
    // Base point (convergence, further from entity)
    let bx = ex + base_offset * dx;
    let by = ey + base_offset * dy;

    // Tip center (closer to entity)
    let tcx = ex + tip_offset * dx;
    let tcy = ey + tip_offset * dy;

    // Two spread tips
    let top_x = tcx + CROW_SPREAD * nx;
    let top_y = tcy + CROW_SPREAD * ny;
    let bot_x = tcx - CROW_SPREAD * nx;
    let bot_y = tcy - CROW_SPREAD * ny;

    // Top prong: base → top tip
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        bx, by, top_x, top_y, color, stroke_width,
    );
    svg.push('\n');

    // Bottom prong: base → bottom tip
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
        bx, by, bot_x, bot_y, color, stroke_width,
    );
    svg.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::er_diagram::{Attribute, AttributeKey, Cardinality, RelationType};
    use crate::layout::er_diagram::types::{
        PositionedEntity, PositionedErDiagram, PositionedRelationship,
    };
    use crate::render::theme::Theme;

    fn make_entity(id: &str, attributes: Vec<Attribute>) -> PositionedEntity {
        PositionedEntity {
            id: id.to_string(),
            alias: None,
            attributes,
            x: 100.0,
            y: 100.0,
            width: 150.0,
            height: 80.0,
            header_height: 30.0,
            col_widths: [40.0, 50.0, 20.0, 0.0],
        }
    }

    fn make_relationship(
        from: &str,
        to: &str,
        card_from: Cardinality,
        card_to: Cardinality,
        rel_type: RelationType,
        label: Option<&str>,
    ) -> PositionedRelationship {
        PositionedRelationship {
            from_id: from.to_string(),
            to_id: to.to_string(),
            cardinality_from: card_from,
            cardinality_to: card_to,
            relation_type: rel_type,
            label: label.map(|s| s.to_string()),
            label_x: label.map(|_| 150.0),
            label_y: label.map(|_| 150.0),
            label_width: label.map(|_| 60.0),
            label_height: label.map(|_| 20.0),
            points: vec![(100.0, 100.0), (200.0, 100.0)],
        }
    }

    fn make_diagram(
        entities: Vec<PositionedEntity>,
        relationships: Vec<PositionedRelationship>,
    ) -> PositionedErDiagram {
        PositionedErDiagram {
            entities,
            relationships,
            width: 400.0,
            height: 300.0,
        }
    }

    #[test]
    fn render_svg_produces_valid_svg_wrapper() {
        let diagram = make_diagram(vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("viewBox="));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn render_svg_includes_style_block() {
        let diagram = make_diagram(vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("<style>"));
        assert!(svg.contains(".er-entity-text"));
        assert!(svg.contains(".er-attr-text"));
        assert!(svg.contains(".er-edge-label"));
    }

    #[test]
    fn render_entity_shows_entity_name() {
        let entity = make_entity("Customer", vec![]);
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("Customer"), "SVG should contain entity name");
    }

    #[test]
    fn render_entity_with_attributes_shows_type_and_name() {
        let entity = make_entity(
            "User",
            vec![
                Attribute {
                    type_name: "int".into(),
                    name: "id".into(),
                    key: AttributeKey::PK,
                    comment: None,
                },
                Attribute {
                    type_name: "string".into(),
                    name: "email".into(),
                    key: AttributeKey::None,
                    comment: Some("user email".into()),
                },
            ],
        );
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("int"), "should contain type name 'int'");
        assert!(svg.contains("id"), "should contain attr name 'id'");
        assert!(svg.contains("PK"), "should show PK key marker");
        assert!(svg.contains("string"), "should contain type name 'string'");
        assert!(svg.contains("email"), "should contain attr name 'email'");
        assert!(svg.contains("user email"), "should contain comment");
    }

    #[test]
    fn render_entity_shows_fk_and_uk_keys() {
        let entity = make_entity(
            "Order",
            vec![
                Attribute {
                    type_name: "int".into(),
                    name: "customer_id".into(),
                    key: AttributeKey::FK,
                    comment: None,
                },
                Attribute {
                    type_name: "string".into(),
                    name: "order_num".into(),
                    key: AttributeKey::UK,
                    comment: None,
                },
            ],
        );
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("FK"), "should show FK key marker");
        assert!(svg.contains("UK"), "should show UK key marker");
    }

    #[test]
    fn render_entity_has_header_divider_line() {
        let entity = make_entity(
            "Product",
            vec![Attribute {
                type_name: "int".into(),
                name: "id".into(),
                key: AttributeKey::PK,
                comment: None,
            }],
        );
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // Divider line between header and attributes
        assert!(
            svg.contains("<line"),
            "should have a divider line after header"
        );
    }

    #[test]
    fn render_entity_no_attributes_no_divider() {
        let entity = make_entity("EmptyEntity", vec![]);
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // Count <line elements - there should be none from entities without attributes
        // (relationships may add lines for cardinality markers, but empty diagram has none)
        let entity_group = svg
            .split("EmptyEntity")
            .nth(1)
            .unwrap_or("")
            .split("</g>")
            .next()
            .unwrap_or("");
        assert!(
            !entity_group.contains("<line"),
            "entity without attributes should not have divider line"
        );
    }

    #[test]
    fn render_relationship_identifying_solid_line() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::OnlyOne,
            Cardinality::ZeroOrMore,
            RelationType::Identifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("<path"), "should have path element for edge");
        assert!(
            !svg.contains("stroke-dasharray"),
            "identifying relationship should have solid line"
        );
    }

    #[test]
    fn render_relationship_non_identifying_dashed_line() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::OnlyOne,
            Cardinality::ZeroOrOne,
            RelationType::NonIdentifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("stroke-dasharray"),
            "non-identifying relationship should have dashed line"
        );
    }

    #[test]
    fn render_relationship_with_label() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::OnlyOne,
            Cardinality::OneOrMore,
            RelationType::Identifying,
            Some("places"),
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("places"), "should contain relationship label");
        // Label should have a background rect
        assert!(
            svg.contains("rgba(232,232,232,0.8)"),
            "label should have a background"
        );
    }

    #[test]
    fn render_relationship_without_label_no_label_text() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::OnlyOne,
            Cardinality::OnlyOne,
            RelationType::Identifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // The class exists in the style block, but no <text class="er-edge-label"> should appear
        // in the rendered content area
        let content_after_style = svg.split("</style>").nth(1).unwrap_or("");
        assert!(
            !content_after_style.contains("er-edge-label"),
            "no label text element should be rendered when relationship has no label"
        );
    }

    #[test]
    fn render_relationship_skips_empty_points() {
        let rel = PositionedRelationship {
            from_id: "A".into(),
            to_id: "B".into(),
            cardinality_from: Cardinality::OnlyOne,
            cardinality_to: Cardinality::OnlyOne,
            relation_type: RelationType::Identifying,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![], // less than 2 points
        };
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // Should not crash, and should not contain a path element for this edge
        assert!(
            !svg.contains("<path"),
            "should skip rendering relationship with < 2 points"
        );
    }

    #[test]
    fn cardinality_only_one_draws_two_perpendicular_lines() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::OnlyOne,
            Cardinality::OnlyOne,
            RelationType::Identifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // OnlyOne (||) should draw perpendicular lines (via <line> elements)
        let line_count = svg.matches("<line").count();
        // Each end draws 2 lines for OnlyOne = 4 total
        assert!(
            line_count >= 4,
            "OnlyOne cardinality on both ends should produce at least 4 <line> elements, got {}",
            line_count
        );
    }

    #[test]
    fn cardinality_zero_or_one_draws_circle() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::ZeroOrOne,
            Cardinality::OnlyOne,
            RelationType::Identifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("<circle"),
            "ZeroOrOne should render a circle element"
        );
    }

    #[test]
    fn cardinality_zero_or_more_draws_circle_and_crows_foot() {
        let rel = make_relationship(
            "A",
            "B",
            Cardinality::ZeroOrMore,
            Cardinality::OnlyOne,
            RelationType::Identifying,
            None,
        );
        let diagram = make_diagram(vec![], vec![rel]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("<circle"),
            "ZeroOrMore should render a circle element"
        );
        // Crow's foot draws 2 line elements (the V prongs)
        let line_count = svg.matches("<line").count();
        assert!(
            line_count >= 4,
            "ZeroOrMore + OnlyOne should produce at least 4 line elements, got {}",
            line_count
        );
    }

    #[test]
    fn render_entity_escapes_xml_in_name() {
        let entity = make_entity("Entity<A>", vec![]);
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("Entity&lt;A&gt;"),
            "entity name should be XML-escaped"
        );
        assert!(
            !svg.contains("Entity<A>"),
            "raw angle brackets should not appear in XML text"
        );
    }

    #[test]
    fn render_alternating_row_backgrounds() {
        let entity = make_entity(
            "Multi",
            vec![
                Attribute {
                    type_name: "int".into(),
                    name: "a".into(),
                    key: AttributeKey::None,
                    comment: None,
                },
                Attribute {
                    type_name: "string".into(),
                    name: "b".into(),
                    key: AttributeKey::None,
                    comment: None,
                },
                Attribute {
                    type_name: "bool".into(),
                    name: "c".into(),
                    key: AttributeKey::None,
                    comment: None,
                },
            ],
        );
        let diagram = make_diagram(vec![entity], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // Count rect elements in the entity group:
        // 1 outer rect + 1 header bg + alternating row bg for odd rows (index 1 = "b")
        let rect_count = svg.matches("<rect").count();
        assert!(
            rect_count >= 3,
            "should have outer rect, header bg, and at least one alternating row bg, got {}",
            rect_count
        );
    }
}
