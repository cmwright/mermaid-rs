use std::collections::HashMap;

use crate::ast::gitgraph::*;
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;
use crate::render::theme::Theme;

// ── Constants ───────────────────────────────────────────────

const COMMIT_SPACING: f64 = 60.0;
const BRANCH_SPACING: f64 = 50.0;
const COMMIT_RADIUS: f64 = 10.0;
const DIAGRAM_PADDING: f64 = 20.0;
const TAG_HEIGHT: f64 = 20.0;
const TAG_PADDING_H: f64 = 8.0;

// ── Output types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GitGraphLayout {
    pub width: f64,
    pub height: f64,
    pub branch_labels: Vec<PositionedBranchLabel>,
    pub branch_lines: Vec<PositionedBranchLine>,
    pub commits: Vec<PositionedCommit>,
    pub connections: Vec<PositionedConnection>,
    pub tags: Vec<PositionedTag>,
}

#[derive(Debug, Clone)]
pub struct PositionedBranchLabel {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedBranchLine {
    pub y: f64,
    pub x_start: f64,
    pub x_end: f64,
    pub color_index: usize,
    pub is_dotted: bool,
}

#[derive(Debug, Clone)]
pub struct PositionedCommit {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub commit_type: CommitType,
    pub color_index: usize,
    pub is_merge: bool,
}

#[derive(Debug, Clone)]
pub struct PositionedConnection {
    pub points: Vec<(f64, f64)>,
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedTag {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

// ── Simulation types ────────────────────────────────────────

struct GitSimulation {
    branches: Vec<BranchInfo>,
    branch_map: HashMap<String, usize>,
    commits: Vec<SimCommit>,
    /// For each branch (by index), the commit index it was branched from
    branch_source_commit: HashMap<usize, usize>,
}

struct BranchInfo {
    name: String,
    color_index: usize,
}

struct SimCommit {
    id: String,
    branch_name: String,
    commit_type: CommitType,
    tag: Option<String>,
    merge_parent: Option<usize>,
    branch_source: Option<usize>,
}

// ── Entry point ─────────────────────────────────────────────

pub fn layout_gitgraph(
    ast: &GitGraphAst,
    measurer: &TextMeasurer,
    _theme: &Theme,
) -> Result<GitGraphLayout> {
    let sim = simulate_git(ast);
    let layout = position_graph(&sim, measurer);
    Ok(layout)
}

// ── Git simulation ──────────────────────────────────────────

fn simulate_git(ast: &GitGraphAst) -> GitSimulation {
    let mut branches = vec![BranchInfo {
        name: "main".to_string(),
        color_index: 0,
    }];
    let mut branch_map: HashMap<String, usize> = HashMap::new();
    branch_map.insert("main".to_string(), 0);

    let mut commits = Vec::new();
    let mut current_branch = "main".to_string();

    let mut branch_heads: HashMap<String, Option<usize>> = HashMap::new();
    branch_heads.insert("main".to_string(), None);

    let mut auto_counter: u32 = 0;
    let mut pending_branch_source: Option<usize> = None;
    let mut branch_source_commit: HashMap<usize, usize> = HashMap::new();

    for cmd in &ast.commands {
        match cmd {
            GitCommand::Commit(def) => {
                let id = if let Some(custom_id) = &def.id {
                    custom_id.clone()
                } else {
                    auto_counter += 1;
                    format!("{:0>7x}", auto_counter)
                };

                commits.push(SimCommit {
                    id,
                    branch_name: current_branch.clone(),
                    commit_type: def.commit_type,
                    tag: def.tag.clone(),
                    merge_parent: None,
                    branch_source: pending_branch_source.take(),
                });

                let commit_idx = commits.len() - 1;
                branch_heads.insert(current_branch.clone(), Some(commit_idx));
            }
            GitCommand::Branch(def) => {
                if !branch_map.contains_key(&def.name) {
                    let color_index = branches.len();
                    branch_map.insert(def.name.clone(), color_index);
                    branches.push(BranchInfo {
                        name: def.name.clone(),
                        color_index,
                    });
                    let head = branch_heads.get(&current_branch).copied().flatten();
                    branch_heads.insert(def.name.clone(), head);
                    if let Some(src) = head {
                        branch_source_commit.insert(color_index, src);
                    }
                    pending_branch_source = head;
                    // In mermaid.js, `branch` also checks out the new branch
                    current_branch = def.name.clone();
                }
            }
            GitCommand::Checkout(def) => {
                // Only clear pending_branch_source if actually switching branches.
                // A `checkout X` right after `branch X` is a no-op and shouldn't
                // clear the source connection for the next commit.
                if def.name != current_branch {
                    pending_branch_source = None;
                }
                current_branch = def.name.clone();
            }
            GitCommand::Merge(def) => {
                let merge_from = branch_heads.get(&def.branch).copied().flatten();
                auto_counter += 1;
                let id = format!("{:0>7x}", auto_counter);

                commits.push(SimCommit {
                    id,
                    branch_name: current_branch.clone(),
                    commit_type: CommitType::Normal,
                    tag: None,
                    merge_parent: merge_from,
                    branch_source: None,
                });

                let commit_idx = commits.len() - 1;
                branch_heads.insert(current_branch.clone(), Some(commit_idx));
            }
        }
    }

    GitSimulation {
        branches,
        branch_map,
        commits,
        branch_source_commit,
    }
}

// ── Positioning ─────────────────────────────────────────────

fn position_graph(sim: &GitSimulation, measurer: &TextMeasurer) -> GitGraphLayout {
    // Measure branch label widths
    let mut max_label_width: f64 = 0.0;
    let label_pad_h = 12.0;
    let mut label_widths: Vec<f64> = Vec::new();
    for branch in &sim.branches {
        let metrics = measurer.measure(&branch.name);
        let w = metrics.width + label_pad_h * 2.0;
        label_widths.push(w);
        max_label_width = max_label_width.max(w);
    }

    // Y-position for each branch lane
    let mut branch_y: HashMap<String, f64> = HashMap::new();
    for (i, branch) in sim.branches.iter().enumerate() {
        branch_y.insert(
            branch.name.clone(),
            DIAGRAM_PADDING + i as f64 * BRANCH_SPACING,
        );
    }

    let x_start = DIAGRAM_PADDING + max_label_width + 20.0;

    // Position commits sequentially
    let mut positioned_commits = Vec::new();
    let mut commit_positions: Vec<(f64, f64)> = Vec::new();

    for (i, c) in sim.commits.iter().enumerate() {
        let x = x_start + i as f64 * COMMIT_SPACING;
        let y = *branch_y.get(&c.branch_name).unwrap_or(&DIAGRAM_PADDING);
        let color_index = sim.branch_map.get(&c.branch_name).copied().unwrap_or(0);

        positioned_commits.push(PositionedCommit {
            id: c.id.clone(),
            x,
            y,
            commit_type: c.commit_type,
            color_index,
            is_merge: c.merge_parent.is_some(),
        });
        commit_positions.push((x, y));
    }

    // Connections (merge + branch-from)
    let mut connections = Vec::new();
    for (i, c) in sim.commits.iter().enumerate() {
        if let Some(mp) = c.merge_parent {
            let (from_x, from_y) = commit_positions[mp];
            let (to_x, to_y) = commit_positions[i];
            let color_index = sim
                .branch_map
                .get(&sim.commits[mp].branch_name)
                .copied()
                .unwrap_or(0);
            connections.push(PositionedConnection {
                points: compute_curve_points(from_x, from_y, to_x, to_y),
                color_index,
            });
        }
        if let Some(bs) = c.branch_source {
            let (from_x, from_y) = commit_positions[bs];
            let (to_x, to_y) = commit_positions[i];
            let color_index = sim.branch_map.get(&c.branch_name).copied().unwrap_or(0);
            connections.push(PositionedConnection {
                points: compute_curve_points(from_x, from_y, to_x, to_y),
                color_index,
            });
        }
    }

    // Tags
    let mut tags = Vec::new();
    for (i, c) in sim.commits.iter().enumerate() {
        if let Some(tag_text) = &c.tag {
            let tag_w = measurer.measure(tag_text).width + TAG_PADDING_H * 2.0;
            tags.push(PositionedTag {
                text: tag_text.clone(),
                x: commit_positions[i].0,
                y: commit_positions[i].1 - COMMIT_RADIUS - TAG_HEIGHT - 2.0,
                width: tag_w,
            });
        }
    }

    // Branch labels
    let mut branch_labels = Vec::new();
    for (i, branch) in sim.branches.iter().enumerate() {
        let y = *branch_y.get(&branch.name).unwrap_or(&0.0);
        branch_labels.push(PositionedBranchLabel {
            name: branch.name.clone(),
            x: DIAGRAM_PADDING,
            y,
            width: label_widths[i],
            color_index: branch.color_index,
        });
    }

    // Branch lines:
    // - For main: solid from first to last commit
    // - For other branches: dotted "dormant" from x_start to branch-off point,
    //   solid from branch-off to last commit, dotted continuation to end
    let last_x = positioned_commits
        .iter()
        .map(|c| c.x)
        .fold(0.0_f64, f64::max);

    let mut branch_lines = Vec::new();
    for branch in &sim.branches {
        let y = *branch_y.get(&branch.name).unwrap_or(&0.0);
        let color_index = branch.color_index;

        let branch_xs: Vec<f64> = positioned_commits
            .iter()
            .filter(|c| c.color_index == color_index)
            .map(|c| c.x)
            .collect();

        let last_commit_x = branch_xs.iter().copied().reduce(f64::max);

        let _branch_off_x = sim
            .branch_source_commit
            .get(&color_index)
            .map(|&idx| commit_positions[idx].0);

        if color_index == 0 {
            // Main branch: solid line spanning all its commits
            if let (Some(first), Some(last)) =
                (branch_xs.iter().copied().reduce(f64::min), last_commit_x)
            {
                branch_lines.push(PositionedBranchLine {
                    y,
                    x_start: first,
                    x_end: last,
                    color_index,
                    is_dotted: false,
                });
                // Dotted after last main commit if other commits follow
                if last < last_x {
                    branch_lines.push(PositionedBranchLine {
                        y,
                        x_start: last,
                        x_end: last_x,
                        color_index,
                        is_dotted: true,
                    });
                }
            }
        } else {
            // Non-main branch: start from first commit, not branch-off point
            // The connection curve handles the path from parent to first commit
            let first_commit_x = branch_xs.iter().copied().reduce(f64::min);

            if let (Some(first_cx), Some(last_cx)) = (first_commit_x, last_commit_x) {
                // Solid from first commit to last commit
                branch_lines.push(PositionedBranchLine {
                    y,
                    x_start: first_cx,
                    x_end: last_cx,
                    color_index,
                    is_dotted: false,
                });

                // Dotted continuation to end
                if last_cx < last_x {
                    branch_lines.push(PositionedBranchLine {
                        y,
                        x_start: last_cx,
                        x_end: last_x,
                        color_index,
                        is_dotted: true,
                    });
                }
            }
        }
    }

    // Bounding box
    let max_x = last_x + COMMIT_SPACING;
    let max_y = sim.branches.len() as f64 * BRANCH_SPACING + DIAGRAM_PADDING;

    GitGraphLayout {
        width: max_x + DIAGRAM_PADDING,
        height: max_y,
        branch_labels,
        branch_lines,
        commits: positioned_commits,
        connections,
        tags,
    }
}

fn compute_curve_points(from_x: f64, from_y: f64, to_x: f64, to_y: f64) -> Vec<(f64, f64)> {
    // Curve stays on source branch lane, then transitions vertically
    // near the target commit (like mermaid.js).
    // Duplicate start/end to pin the B-spline endpoints.
    let dx = (to_x - from_x).abs();
    let bend_x = to_x - dx.min(COMMIT_SPACING * 0.6);
    vec![
        (from_x, from_y),
        (from_x, from_y),
        (bend_x, from_y),
        (to_x, to_y),
        (to_x, to_y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;
    use crate::render::theme::Theme;

    fn make_measurer() -> (FontProvider, Theme) {
        (FontProvider::default_font(), Theme::default())
    }

    #[test]
    fn test_layout_simple_gitgraph() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.commits.len(), 2);
        assert_eq!(layout.branch_labels.len(), 1);
        assert_eq!(layout.branch_labels[0].name, "main");
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
        // Second commit should be to the right of the first
        assert!(layout.commits[1].x > layout.commits[0].x);
    }

    #[test]
    fn test_layout_gitgraph_branch_and_merge() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Checkout(CheckoutDef {
                    name: "main".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c3".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Merge(MergeDef {
                    branch: "feature".to_string(),
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        // 4 commits: c1, c2 (on feature), c3 (on main), merge commit
        assert_eq!(layout.commits.len(), 4);
        // 2 branches: main and feature
        assert_eq!(layout.branch_labels.len(), 2);
        assert!(layout.branch_labels.iter().any(|b| b.name == "main"));
        assert!(layout.branch_labels.iter().any(|b| b.name == "feature"));
        // Merge commit should be flagged
        assert!(layout.commits[3].is_merge);
        // Should have a merge connection
        assert!(
            !layout.connections.is_empty(),
            "Should have at least one connection for the merge"
        );
    }

    #[test]
    fn test_layout_gitgraph_multiple_branches() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "develop".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.branch_labels.len(), 3);
        assert_eq!(layout.commits.len(), 3);
        // Each branch should be at a different y position
        let ys: Vec<f64> = layout.branch_labels.iter().map(|b| b.y).collect();
        assert!(ys[0] < ys[1], "Branches should be at different y levels");
        assert!(ys[1] < ys[2], "Branches should be at different y levels");
    }

    #[test]
    fn test_layout_gitgraph_with_tags() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: Some("v1.0".to_string()),
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.tags.len(), 1);
        assert_eq!(layout.tags[0].text, "v1.0");
        // Tag should be above the commit
        assert!(layout.tags[0].y < layout.commits[0].y);
    }

    #[test]
    fn test_layout_gitgraph_commit_types() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Reverse,
                }),
                GitCommand::Commit(CommitDef {
                    id: None,
                    message: None,
                    tag: None,
                    commit_type: CommitType::Highlight,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.commits.len(), 3);
        assert_eq!(layout.commits[0].commit_type, CommitType::Normal);
        assert_eq!(layout.commits[1].commit_type, CommitType::Reverse);
        assert_eq!(layout.commits[2].commit_type, CommitType::Highlight);
    }

    #[test]
    fn test_layout_gitgraph_branch_lines() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Checkout(CheckoutDef {
                    name: "main".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c3".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        // Should have branch lines for both branches
        assert!(
            !layout.branch_lines.is_empty(),
            "Should have branch lines"
        );
        // Main branch should have a solid line
        let main_solid = layout.branch_lines.iter().any(|l| l.color_index == 0 && !l.is_dotted);
        assert!(main_solid, "Main branch should have a solid line");
    }

    #[test]
    fn test_layout_gitgraph_non_main_dotted_continuation() {
        // Feature branch has commits, then main has more. Feature's last commit < last_x.
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Checkout(CheckoutDef {
                    name: "main".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c3".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        // Feature (color_index 1) has only c2; main has c1 and c3. last_x = c3.
        // Feature's last_cx = c2 < last_x, so feature gets dotted continuation.
        let feature_dotted = layout
            .branch_lines
            .iter()
            .any(|l| l.color_index == 1 && l.is_dotted);
        assert!(
            feature_dotted,
            "Feature branch should have dotted continuation when main has later commits"
        );
    }

    #[test]
    fn test_layout_gitgraph_main_dotted_continuation() {
        // Main has 1 commit, then switch to feature for 2 more commits.
        // Main's last commit is at x < last_x, so it should get a dotted continuation.
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c3".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        // Main has only c1, feature has c2 and c3.
        // Main's branch line should include a dotted continuation.
        let main_dotted = layout
            .branch_lines
            .iter()
            .any(|l| l.color_index == 0 && l.is_dotted);
        assert!(
            main_dotted,
            "Main branch should have a dotted continuation line when other branches have later commits"
        );
    }

    #[test]
    fn test_layout_gitgraph_checkout_same_branch_noop() {
        let ast = GitGraphAst {
            commands: vec![
                GitCommand::Commit(CommitDef {
                    id: Some("c1".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
                GitCommand::Branch(BranchDef {
                    name: "feature".to_string(),
                }),
                // checkout feature right after branch feature is a no-op
                GitCommand::Checkout(CheckoutDef {
                    name: "feature".to_string(),
                }),
                GitCommand::Commit(CommitDef {
                    id: Some("c2".to_string()),
                    message: None,
                    tag: None,
                    commit_type: CommitType::Normal,
                }),
            ],
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        // c2 should be on feature branch and should have a branch_source connection
        assert_eq!(layout.commits.len(), 2);
        assert!(!layout.connections.is_empty(), "should have a branch-from connection");
    }

    #[test]
    fn test_layout_gitgraph_empty() {
        let ast = GitGraphAst {
            commands: Vec::new(),
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_gitgraph(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.commits.len(), 0);
        // Still has main branch label
        assert_eq!(layout.branch_labels.len(), 1);
    }
}
