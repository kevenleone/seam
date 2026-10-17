//! Patches that contain only some of a diff's changed lines, for staging, unstaging or
//! discarding individual lines and hunks.

use std::collections::{HashMap, HashSet};

use super::models::{FileDiff, Hunk, LineKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Applied forwards onto the diff's old side (staging work-tree changes).
    Forward,
    /// Applied with `--reverse` onto the diff's new side (unstaging, discarding).
    Reverse,
}

/// Line ids to include, per hunk id. A hunk missing from the map is left out.
pub type Selection = HashMap<usize, HashSet<usize>>;

/// `selection` None means every changed line of every hunk.
pub fn build(
    file: &FileDiff,
    selection: Option<&Selection>,
    direction: Direction,
) -> Option<String> {
    let mut hunk_texts = Vec::new();
    let mut everything = true;

    for hunk in &file.hunks {
        let selected: HashSet<usize> = match selection {
            Some(sel) => match sel.get(&hunk.id) {
                Some(s) if !s.is_empty() => s.clone(),
                _ => {
                    if hunk.lines.iter().any(|l| l.is_change()) {
                        everything = false;
                    }
                    continue;
                }
            },
            None => hunk
                .lines
                .iter()
                .filter(|l| l.is_change())
                .map(|l| l.id)
                .collect(),
        };
        if hunk
            .lines
            .iter()
            .any(|l| l.is_change() && !selected.contains(&l.id))
        {
            everything = false;
        }
        if let Some(text) = hunk_text(hunk, &selected, direction) {
            hunk_texts.push(text);
        }
    }
    if hunk_texts.is_empty() {
        return None;
    }
    let header = if everything {
        file.header_lines.clone()
    } else {
        partial_header(file, direction)
    };
    let mut out = header.join("\n");
    out.push('\n');
    out.push_str(&hunk_texts.join("\n"));
    out.push('\n');
    Some(out)
}

fn hunk_text(hunk: &Hunk, selected: &HashSet<usize>, direction: Direction) -> Option<String> {
    let mut out = Vec::new();
    let (mut old_count, mut new_count) = (0u32, 0u32);
    let mut has_change = false;
    let mut last_kept = false;

    for line in &hunk.lines {
        let is_selected = selected.contains(&line.id);
        match line.kind {
            LineKind::Context => {
                out.push(format!(" {}", line.text));
                old_count += 1;
                new_count += 1;
                last_kept = true;
            }
            LineKind::Addition => {
                if is_selected {
                    out.push(format!("+{}", line.text));
                    new_count += 1;
                    has_change = true;
                    last_kept = true;
                } else if direction == Direction::Reverse {
                    // Present on the side we apply to; keep it as context.
                    out.push(format!(" {}", line.text));
                    old_count += 1;
                    new_count += 1;
                    last_kept = true;
                } else {
                    last_kept = false;
                }
            }
            LineKind::Deletion => {
                if is_selected {
                    out.push(format!("-{}", line.text));
                    old_count += 1;
                    has_change = true;
                    last_kept = true;
                } else if direction == Direction::Forward {
                    out.push(format!(" {}", line.text));
                    old_count += 1;
                    new_count += 1;
                    last_kept = true;
                } else {
                    last_kept = false;
                }
            }
            LineKind::NoNewline => {
                if last_kept {
                    out.push(line.text.clone());
                }
            }
        }
    }
    if !has_change {
        return None;
    }
    // The side the patch is applied to keeps all of its lines, so its start and count match
    // the original hunk. The other side's start only has to be plausible: git apply tolerates
    // the offsets left by lines and hunks we dropped, and --recount fixes the counts.
    let (applied_start, applied_count, other_count) = match direction {
        Direction::Forward => (hunk.old_start, old_count, new_count),
        Direction::Reverse => (hunk.new_start, new_count, old_count),
    };
    let mut other_start = applied_start;
    if applied_count == 0 && other_count > 0 {
        other_start = applied_start + 1;
    }
    if applied_count > 0 && other_count == 0 {
        other_start = applied_start.saturating_sub(1);
    }
    let (old_start, new_start) = match direction {
        Direction::Forward => (applied_start, other_start),
        Direction::Reverse => (other_start, applied_start),
    };
    out.insert(
        0,
        format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@"),
    );
    Some(out.join("\n"))
}

/// A partial deletion can't carry "deleted file mode", and neither can a partial new file
/// applied in reverse; rewrite those headers as plain modifications.
fn partial_header(file: &FileDiff, direction: Direction) -> Vec<String> {
    let header = &file.header_lines;
    let is_deletion = header.iter().any(|l| l.starts_with("deleted file mode"));
    let is_new = header.iter().any(|l| l.starts_with("new file mode"));
    if !(is_deletion || (is_new && direction == Direction::Reverse)) {
        return header.clone();
    }
    let path = file.path();
    header
        .iter()
        .filter(|l| {
            !l.starts_with("deleted file mode")
                && !l.starts_with("new file mode")
                && !l.starts_with("index ")
        })
        .map(|l| {
            if l.starts_with("--- ") {
                format!("--- a/{path}")
            } else if l.starts_with("+++ ") {
                format!("+++ b/{path}")
            } else {
                l.clone()
            }
        })
        .collect()
}
