//! A simple 2D character canvas for rendering diagrams as Unicode text art.
//!
//! Provides drawing primitives (boxes, lines, arrows, text) on a grid of characters.
//! Coordinates are in pixel space (matching the positioned layout types) and are
//! automatically scaled to character cells.

/// Scale factors for converting pixel coordinates to character cells.
/// Monospace characters are roughly twice as tall as they are wide.
const PX_PER_CHAR_X: f64 = 8.0;
const PX_PER_CHAR_Y: f64 = 14.0;

/// Padding added around the canvas edges (in character cells).
const CANVAS_PADDING: usize = 1;

/// A 2D grid of characters that can be rendered to a string.
#[derive(Debug, Clone)]
pub struct TextCanvas {
    width: usize,
    height: usize,
    cells: Vec<Vec<char>>,
}

impl TextCanvas {
    /// Create a new canvas from pixel dimensions. The canvas is auto-scaled to
    /// character cells and padded.
    pub fn from_pixel_size(px_width: f64, px_height: f64) -> Self {
        let w = (px_width / PX_PER_CHAR_X).ceil() as usize + CANVAS_PADDING * 2;
        let h = (px_height / PX_PER_CHAR_Y).ceil() as usize + CANVAS_PADDING * 2;
        // Clamp to reasonable sizes
        let w = w.max(4).min(500);
        let h = h.max(4).min(200);
        Self {
            width: w,
            height: h,
            cells: vec![vec![' '; w]; h],
        }
    }

    /// Convert pixel X coordinate to column index.
    #[inline]
    pub fn px_to_col(&self, px: f64) -> usize {
        let col = (px / PX_PER_CHAR_X).round() as isize + CANVAS_PADDING as isize;
        col.clamp(0, self.width as isize - 1) as usize
    }

    /// Convert pixel Y coordinate to row index.
    #[inline]
    pub fn px_to_row(&self, py: f64) -> usize {
        let row = (py / PX_PER_CHAR_Y).round() as isize + CANVAS_PADDING as isize;
        row.clamp(0, self.height as isize - 1) as usize
    }

    /// Set a single character at (col, row), if in bounds.
    pub fn put(&mut self, col: usize, row: usize, ch: char) {
        if row < self.height && col < self.width {
            self.cells[row][col] = ch;
        }
    }

    /// Get the character at (col, row), or space if out of bounds.
    pub fn get(&self, col: usize, row: usize) -> char {
        if row < self.height && col < self.width {
            self.cells[row][col]
        } else {
            ' '
        }
    }

    /// Draw a horizontal line from col1 to col2 at row, using the given character.
    pub fn draw_hline(&mut self, col1: usize, col2: usize, row: usize, ch: char) {
        let (lo, hi) = if col1 <= col2 {
            (col1, col2)
        } else {
            (col2, col1)
        };
        for c in lo..=hi {
            self.put(c, row, ch);
        }
    }

    /// Draw a vertical line from row1 to row2 at col, using the given character.
    pub fn draw_vline(&mut self, col: usize, row1: usize, row2: usize, ch: char) {
        let (lo, hi) = if row1 <= row2 {
            (row1, row2)
        } else {
            (row2, row1)
        };
        for r in lo..=hi {
            self.put(col, r, ch);
        }
    }

    /// Draw a box with Unicode box-drawing characters.
    /// (x, y) is the top-left corner in pixel coordinates.
    /// (w, h) are pixel dimensions.
    pub fn draw_box_px(&mut self, px_x: f64, px_y: f64, px_w: f64, px_h: f64) {
        let left = self.px_to_col(px_x);
        let top = self.px_to_row(px_y);
        let right = self.px_to_col(px_x + px_w);
        let bottom = self.px_to_row(px_y + px_h);
        self.draw_box(left, top, right, bottom);
    }

    /// Draw a box with Unicode box-drawing characters at cell coordinates.
    pub fn draw_box(&mut self, left: usize, top: usize, right: usize, bottom: usize) {
        if right <= left || bottom <= top {
            return;
        }
        // Corners
        self.put(left, top, '┌');
        self.put(right, top, '┐');
        self.put(left, bottom, '└');
        self.put(right, bottom, '┘');
        // Horizontal edges
        for c in (left + 1)..right {
            self.put(c, top, '─');
            self.put(c, bottom, '─');
        }
        // Vertical edges
        for r in (top + 1)..bottom {
            self.put(left, r, '│');
            self.put(right, r, '│');
        }
    }

    /// Draw a rounded box (using ╭╮╰╯ corners).
    pub fn draw_rounded_box(&mut self, left: usize, top: usize, right: usize, bottom: usize) {
        if right <= left || bottom <= top {
            return;
        }
        self.put(left, top, '╭');
        self.put(right, top, '╮');
        self.put(left, bottom, '╰');
        self.put(right, bottom, '╯');
        for c in (left + 1)..right {
            self.put(c, top, '─');
            self.put(c, bottom, '─');
        }
        for r in (top + 1)..bottom {
            self.put(left, r, '│');
            self.put(right, r, '│');
        }
    }

    /// Draw a diamond shape at cell coordinates using a clean representation.
    /// Uses ◆ markers at the cardinal points and lines connecting them.
    pub fn draw_diamond(
        &mut self,
        center_col: usize,
        center_row: usize,
        half_w: usize,
        half_h: usize,
    ) {
        // For small diamonds, just draw the center marker
        if half_w <= 1 || half_h == 0 {
            self.put(center_col, center_row, '◇');
            return;
        }

        // Draw the four sides using / and \ characters
        for i in 0..=half_h {
            // Width at this row: proportional to distance from top/bottom
            let w_at_row = ((half_w as f64) * (i as f64 / half_h as f64)).round() as usize;
            if i == 0 {
                // Top point
                self.put(center_col, center_row.saturating_sub(half_h), '◇');
            } else if i == half_h {
                // Middle row: widest points
                if center_col >= half_w {
                    self.put(center_col - half_w, center_row, '◁');
                }
                self.put(center_col + half_w, center_row, '▷');
                // Fill middle with spaces (clear any edge lines passing through)
            } else {
                // Upper half
                let row_up = center_row.saturating_sub(half_h - i);
                if center_col >= w_at_row {
                    self.put(center_col - w_at_row, row_up, '/');
                }
                self.put(center_col + w_at_row, row_up, '\\');
                // Horizontal line between the diagonals
                if w_at_row > 1 {
                    for c in (center_col.saturating_sub(w_at_row) + 1)..(center_col + w_at_row) {
                        // Only draw if currently empty
                        if self.get(c, row_up) == ' ' {
                            self.put(c, row_up, ' ');
                        }
                    }
                }

                // Lower half (mirror)
                let row_down = center_row + (half_h - i);
                if center_col >= w_at_row {
                    self.put(center_col - w_at_row, row_down, '\\');
                }
                self.put(center_col + w_at_row, row_down, '/');
            }
        }
        // Bottom point
        self.put(center_col, center_row + half_h, '◇');
    }

    /// Place a text string horizontally, centered at the given pixel position.
    pub fn draw_text_centered_px(&mut self, px_x: f64, px_y: f64, text: &str) {
        let col = self.px_to_col(px_x);
        let row = self.px_to_row(px_y);
        let len = text.chars().count();
        let start_col = col.saturating_sub(len / 2);
        for (i, ch) in text.chars().enumerate() {
            self.put(start_col + i, row, ch);
        }
    }

    /// Place a text string starting at (col, row).
    pub fn draw_text(&mut self, col: usize, row: usize, text: &str) {
        for (i, ch) in text.chars().enumerate() {
            self.put(col + i, row, ch);
        }
    }

    /// Draw a polyline through pixel-coordinate waypoints.
    ///
    /// The points are first snapped to the character grid, simplified to remove
    /// redundant collinear points, and any remaining diagonal segments are
    /// orthogonalized into clean L-shaped bends. This produces much cleaner
    /// output than trying to draw diagonal lines in a character grid.
    pub fn draw_polyline(&mut self, points: &[(f64, f64)]) {
        let final_pts = self.orthogonalize_points(points);
        if final_pts.len() < 2 {
            return;
        }

        // Draw all segments
        for window in final_pts.windows(2) {
            let (c1, r1) = window[0];
            let (c2, r2) = window[1];
            if r1 == r2 {
                self.draw_hline(c1, c2, r1, '─');
            } else if c1 == c2 {
                self.draw_vline(c1, r1, r2, '│');
            }
        }

        // Draw corners at waypoints
        for i in 1..final_pts.len() - 1 {
            let (c, r) = final_pts[i];
            let (cp, rp) = final_pts[i - 1];
            let (cn, rn) = final_pts[i + 1];
            let corner = Self::pick_corner(cp, rp, c, r, cn, rn);
            self.put(c, r, corner);
        }
    }

    /// Snap pixel points to grid, simplify, and orthogonalize diagonal segments.
    /// Returns a list of grid-coordinate points with only horizontal/vertical segments.
    fn orthogonalize_points(&self, points: &[(f64, f64)]) -> Vec<(usize, usize)> {
        if points.len() < 2 {
            return vec![];
        }

        // Step 1: Snap all points to grid coordinates
        let grid_points: Vec<(usize, usize)> = points
            .iter()
            .map(|(x, y)| (self.px_to_col(*x), self.px_to_row(*y)))
            .collect();

        // Step 2: Deduplicate consecutive identical points
        let mut deduped: Vec<(usize, usize)> = Vec::with_capacity(grid_points.len());
        for &pt in &grid_points {
            if deduped.last() != Some(&pt) {
                deduped.push(pt);
            }
        }

        if deduped.len() < 2 {
            return deduped;
        }

        // Step 3: Remove collinear intermediate points (same row or same col)
        let mut simplified: Vec<(usize, usize)> = Vec::with_capacity(deduped.len());
        simplified.push(deduped[0]);
        for i in 1..deduped.len() - 1 {
            let (pc, pr) = simplified[simplified.len() - 1];
            let (cc, cr) = deduped[i];
            let (nc, nr) = deduped[i + 1];
            // Keep this point only if direction changes
            let same_col = pc == cc && cc == nc;
            let same_row = pr == cr && cr == nr;
            if !same_col && !same_row {
                simplified.push((cc, cr));
            }
        }
        simplified.push(*deduped.last().unwrap());

        // Step 4: Orthogonalize any remaining diagonal segments
        // Convert diagonal (c1,r1)->(c2,r2) into L-shaped bends.
        let mut ortho: Vec<(usize, usize)> = Vec::with_capacity(simplified.len() * 2);
        ortho.push(simplified[0]);
        for i in 1..simplified.len() {
            let (pc, pr) = ortho[ortho.len() - 1];
            let (nc, nr) = simplified[i];
            if pc != nc && pr != nr {
                // Diagonal segment: insert an intermediate corner point.
                // Choose horizontal-first or vertical-first based on which
                // axis has more travel distance.
                let dc = (nc as isize - pc as isize).unsigned_abs();
                let dr = (nr as isize - pr as isize).unsigned_abs();
                if dc >= dr {
                    // Horizontal first, then vertical
                    ortho.push((nc, pr));
                } else {
                    // Vertical first, then horizontal
                    ortho.push((pc, nr));
                }
            }
            ortho.push((nc, nr));
        }

        // Step 5: Collapse short jogs. A "jog" is a tiny segment (1-2 cells)
        // sandwiched between two parallel segments going the same direction.
        // For example: down, right 1, down => collapse to just down.
        let mut collapsed: Vec<(usize, usize)> = Vec::with_capacity(ortho.len());
        collapsed.push(ortho[0]);
        let mut i = 1;
        while i < ortho.len() {
            if i + 1 < ortho.len() {
                let (pc, pr) = collapsed[collapsed.len() - 1];
                let (cc, cr) = ortho[i];
                let (nc, nr) = ortho[i + 1];

                // Check for horizontal jog between two vertical segments
                // prev->cur is horizontal, cur->next is vertical, and prev and next
                // are both on the same column axis (vertical continuity)
                let is_h_jog = pr == cr && cc == nc && pr != nr;
                let h_jog_len = if is_h_jog {
                    (cc as isize - pc as isize).unsigned_abs()
                } else {
                    0
                };

                // Check for vertical jog between two horizontal segments
                let is_v_jog = pc == cc && cr == nr && pc != nc;
                let v_jog_len = if is_v_jog {
                    (cr as isize - pr as isize).unsigned_abs()
                } else {
                    0
                };

                if is_h_jog && h_jog_len <= 2 {
                    // Collapse: skip the intermediate point, connect prev directly
                    // to next on the prev's column
                    collapsed.push((pc, nr));
                    i += 2;
                    continue;
                }
                if is_v_jog && v_jog_len <= 2 {
                    // Collapse: skip the intermediate point, connect prev directly
                    // to next on the prev's row
                    collapsed.push((nc, pr));
                    i += 2;
                    continue;
                }
            }
            collapsed.push(ortho[i]);
            i += 1;
        }

        // Step 6: Final dedup pass (collapsing can create duplicates)
        let mut final_pts: Vec<(usize, usize)> = Vec::with_capacity(collapsed.len());
        for &pt in &collapsed {
            if final_pts.last() != Some(&pt) {
                final_pts.push(pt);
            }
        }

        final_pts
    }

    /// Pick an appropriate corner/junction character for a waypoint.
    fn pick_corner(
        c_prev: usize,
        r_prev: usize,
        c: usize,
        r: usize,
        c_next: usize,
        r_next: usize,
    ) -> char {
        let from_left = c_prev < c;
        let from_right = c_prev > c;
        let from_above = r_prev < r;
        let from_below = r_prev > r;
        let to_right = c_next > c;
        let to_left = c_next < c;
        let to_below = r_next > r;
        let to_above = r_next < r;

        // Straight-through: no corner needed
        if (from_left || from_right) && (to_left || to_right) && r_prev == r && r == r_next {
            return '─';
        }
        if (from_above || from_below) && (to_above || to_below) && c_prev == c && c == c_next {
            return '│';
        }

        // L-shaped corners (using round corners for smoother look)
        if (from_left && to_below) || (from_above && to_right) {
            return '╭';
        }
        if (from_right && to_below) || (from_above && to_left) {
            return '╮';
        }
        if (from_left && to_above) || (from_below && to_right) {
            return '╰';
        }
        if (from_right && to_above) || (from_below && to_left) {
            return '╯';
        }

        '+'
    }

    /// Draw an arrowhead at a pixel position, pointing in a direction.
    pub fn draw_arrow(&mut self, px_x: f64, px_y: f64, direction: ArrowDirection) {
        let c = self.px_to_col(px_x);
        let r = self.px_to_row(px_y);
        let ch = match direction {
            ArrowDirection::Right => '▶',
            ArrowDirection::Left => '◀',
            ArrowDirection::Down => '▼',
            ArrowDirection::Up => '▲',
        };
        self.put(c, r, ch);
    }

    /// Draw a dashed polyline through pixel-coordinate waypoints.
    /// Same orthogonalization as `draw_polyline`, but with dashed line characters.
    pub fn draw_polyline_dashed(&mut self, points: &[(f64, f64)]) {
        let ortho = self.orthogonalize_points(points);
        if ortho.len() < 2 {
            return;
        }
        for window in ortho.windows(2) {
            let (c1, r1) = window[0];
            let (c2, r2) = window[1];
            if r1 == r2 {
                self.draw_dashed_hline(c1, c2, r1);
            } else if c1 == c2 {
                self.draw_dashed_vline(c1, r1, r2);
            }
        }
        for i in 1..ortho.len() - 1 {
            let (c, r) = ortho[i];
            let (cp, rp) = ortho[i - 1];
            let (cn, rn) = ortho[i + 1];
            let corner = Self::pick_corner(cp, rp, c, r, cn, rn);
            self.put(c, r, corner);
        }
    }

    /// Draw a thick polyline through pixel-coordinate waypoints.
    /// Same orthogonalization as `draw_polyline`, but with thick line characters.
    pub fn draw_polyline_thick(&mut self, points: &[(f64, f64)]) {
        let ortho = self.orthogonalize_points(points);
        if ortho.len() < 2 {
            return;
        }
        for window in ortho.windows(2) {
            let (c1, r1) = window[0];
            let (c2, r2) = window[1];
            if r1 == r2 {
                self.draw_hline(c1, c2, r1, '━');
            } else if c1 == c2 {
                self.draw_vline(c1, r1, r2, '┃');
            }
        }
        for i in 1..ortho.len() - 1 {
            let (c, r) = ortho[i];
            let (cp, rp) = ortho[i - 1];
            let (cn, rn) = ortho[i + 1];
            let corner = Self::pick_corner(cp, rp, c, r, cn, rn);
            self.put(c, r, corner);
        }
    }

    /// Draw a dashed horizontal line.
    pub fn draw_dashed_hline(&mut self, col1: usize, col2: usize, row: usize) {
        let (lo, hi) = if col1 <= col2 {
            (col1, col2)
        } else {
            (col2, col1)
        };
        for c in lo..=hi {
            let ch = if (c - lo) % 2 == 0 { '╌' } else { ' ' };
            self.put(c, row, ch);
        }
    }

    /// Draw a dashed vertical line.
    pub fn draw_dashed_vline(&mut self, col: usize, row1: usize, row2: usize) {
        let (lo, hi) = if row1 <= row2 {
            (row1, row2)
        } else {
            (row2, row1)
        };
        for r in lo..=hi {
            let ch = if (r - lo) % 2 == 0 { '╎' } else { ' ' };
            self.put(col, r, ch);
        }
    }

    /// Flatten the canvas to a string, trimming trailing whitespace per line
    /// and trailing blank lines.
    pub fn to_string(&self) -> String {
        let mut lines: Vec<String> = self
            .cells
            .iter()
            .map(|row| {
                let s: String = row.iter().collect();
                s.trim_end().to_string()
            })
            .collect();

        // Remove trailing blank lines
        while lines.last().map_or(false, |l| l.is_empty()) {
            lines.pop();
        }

        lines.join("\n")
    }
}

/// Direction for arrowheads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_creation() {
        let c = TextCanvas::from_pixel_size(80.0, 56.0);
        assert!(c.width >= 10);
        assert!(c.height >= 4);
    }

    #[test]
    fn test_draw_box() {
        let mut c = TextCanvas::from_pixel_size(80.0, 56.0);
        c.draw_box(1, 1, 5, 3);
        assert_eq!(c.get(1, 1), '┌');
        assert_eq!(c.get(5, 1), '┐');
        assert_eq!(c.get(1, 3), '└');
        assert_eq!(c.get(5, 3), '┘');
        assert_eq!(c.get(3, 1), '─');
        assert_eq!(c.get(1, 2), '│');
    }

    #[test]
    fn test_draw_text() {
        let mut c = TextCanvas::from_pixel_size(80.0, 56.0);
        c.draw_text(2, 2, "Hello");
        assert_eq!(c.get(2, 2), 'H');
        assert_eq!(c.get(6, 2), 'o');
    }

    #[test]
    fn test_to_string_trims() {
        let mut c = TextCanvas::from_pixel_size(40.0, 28.0);
        c.put(1, 1, 'X');
        let s = c.to_string();
        // Should not have trailing spaces or trailing blank lines
        for line in s.lines() {
            assert!(
                !line.ends_with(' '),
                "Line should not end with space: {:?}",
                line
            );
        }
        assert!(!s.ends_with('\n'));
    }
}
