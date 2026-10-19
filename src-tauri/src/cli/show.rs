use std::io::{IsTerminal, Write};
use std::process::{Child, Stdio};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::args::{Layout, Show};
use super::highlight::{Highlighter, Palette, Span};
use super::term::{self, Ink, Rgb, Tone};
use crate::git::models::{DiffLine, FileDiff, Hunk, LineKind};
use crate::git::parse;
use crate::git::process::RunOptions;
use crate::git::Repo;

struct Commit {
    sha: String,
    refs: String,
    author: String,
    email: String,
    date: String,
    relative: String,
    message: String,
}

pub async fn run(show: Show) -> Result<i32, String> {
    let (repo, _) = super::repository().await?;
    let commit = commit(&repo, &show.revision).await?;
    let mut arguments = vec![
        "show",
        "--diff-merges=first-parent",
        "--format=",
        "--no-color",
        "--no-ext-diff",
        &commit.sha,
        "--",
    ];
    arguments.extend(show.paths.iter().map(String::as_str));
    let diffs = parse::diff(&read(&repo, &arguments).await?);

    let layout = show
        .layout
        .or_else(|| match std::env::var("SEAM_LAYOUT").as_deref() {
            Ok("split") => Some(Layout::Split),
            Ok("unified") => Some(Layout::Unified),
            _ => None,
        })
        .unwrap_or(Layout::Unified);
    let ink = Ink::stdout();
    let background = if ink.on { term::background() } else { None };
    let palette = Palette::new(background, ink.truecolor);
    let highlighter = ink.on.then(Highlighter::new);
    let page = Page {
        ink,
        palette: &palette,
        highlighter: highlighter.as_ref(),
        width: term::terminal_width().unwrap_or(120).max(40),
        layout,
    };

    let mut out = page.header(&commit, &diffs);
    for diff in &diffs {
        page.file(&mut out, diff);
    }
    show_paged(&out, show.pager);
    Ok(0)
}

async fn read(repo: &Repo, arguments: &[&str]) -> Result<String, String> {
    repo.run_with(
        arguments,
        RunOptions {
            read: true,
            ..Default::default()
        },
    )
    .await
    .map(|output| output.text())
    .map_err(|error| error.message)
}

async fn commit(repo: &Repo, revision: &str) -> Result<Commit, String> {
    let commit = format!("{revision}^{{commit}}");
    let sha = repo
        .run_with(
            &["rev-parse", "--verify", "--quiet", &commit],
            RunOptions {
                read: true,
                allow_failure: true,
                ..Default::default()
            },
        )
        .await
        .map_err(|error| error.message)?;
    if sha.code != 0 {
        return Err(format!("`{revision}` isn't a commit in this repository"));
    }
    let sha = sha.text().trim().to_string();
    let text = read(
        repo,
        &[
            "show",
            "-s",
            "--no-color",
            "--date=format-local:%a %d %b %Y %H:%M",
            "--format=%H%x1F%D%x1F%an%x1F%ae%x1F%ad%x1F%ar%x1F%B",
            &sha,
        ],
    )
    .await?;
    let mut fields = text.splitn(7, '\x1F').map(str::to_string);
    let mut next = || fields.next().unwrap_or_default();
    Ok(Commit {
        sha: next(),
        refs: next(),
        author: next(),
        email: next(),
        date: next(),
        relative: next(),
        message: next().trim_end().to_string(),
    })
}

fn show_paged(text: &str, pager: bool) {
    if pager && std::io::stdout().is_terminal() {
        if let Some(mut child) = pager_process() {
            if let Some(mut input) = child.stdin.take() {
                let _ = input.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }
    let _ = std::io::stdout().write_all(text.as_bytes());
}

/// `SEAM_PAGER`, then `PAGER`, then `less`, the way git and bat pick theirs.
fn pager_process() -> Option<Child> {
    let command = ["SEAM_PAGER", "PAGER"]
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "less".into());
    if command.trim() == "cat" {
        return None;
    }
    let mut process = std::process::Command::new("/bin/sh");
    process.arg("-c").arg(&command).stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        process.env("LESS", "FRX");
    }
    process.spawn().ok()
}

/// Tabs to spaces, and control characters (escape sequences in the content) made visible.
fn printable(text: &str) -> String {
    text.trim_end_matches('\r')
        .chars()
        .flat_map(|character| match character {
            '\t' => vec![' '; 4],
            _ if character.is_control() => vec!['\u{FFFD}'],
            _ => vec![character],
        })
        .collect()
}

fn wrap(spans: &[Span], width: usize) -> Vec<Vec<Span>> {
    let mut rows: Vec<Vec<Span>> = vec![vec![]];
    let mut used = 0;
    for span in spans {
        let mut piece = String::new();
        for character in span.text.chars() {
            let size = character.width().unwrap_or(0);
            if used + size > width && used > 0 {
                if !piece.is_empty() {
                    let text = std::mem::take(&mut piece);
                    rows.last_mut().unwrap().push(Span {
                        text,
                        ..span.clone()
                    });
                }
                rows.push(vec![]);
                used = 0;
            }
            piece.push(character);
            used += size;
        }
        if !piece.is_empty() {
            rows.last_mut().unwrap().push(Span {
                text: piece,
                ..span.clone()
            });
        }
    }
    rows
}

/// Each run of deletions beside the additions that follow it, as the app's split view pairs them.
fn split_rows(hunk: &Hunk) -> Vec<(Option<usize>, Option<usize>)> {
    let mut rows = vec![];
    let mut deletions: Vec<usize> = vec![];
    let mut additions: Vec<usize> = vec![];
    let flush = |rows: &mut Vec<_>, deletions: &mut Vec<usize>, additions: &mut Vec<usize>| {
        for index in 0..deletions.len().max(additions.len()) {
            rows.push((deletions.get(index).copied(), additions.get(index).copied()));
        }
        deletions.clear();
        additions.clear();
    };
    for (index, line) in hunk.lines.iter().enumerate() {
        match line.kind {
            LineKind::Deletion => {
                if !additions.is_empty() {
                    flush(&mut rows, &mut deletions, &mut additions);
                }
                deletions.push(index);
            }
            LineKind::Addition => additions.push(index),
            LineKind::Context => {
                flush(&mut rows, &mut deletions, &mut additions);
                rows.push((Some(index), Some(index)));
            }
            LineKind::NoNewline => {}
        }
    }
    flush(&mut rows, &mut deletions, &mut additions);
    rows
}

struct Tint {
    sign: char,
    sign_color: Tone,
    number: Tone,
    gutter: Option<Rgb>,
    background: Option<Rgb>,
}

struct Cell<'a> {
    gutter: String,
    kind: LineKind,
    spans: &'a [Span],
}

struct Page<'a> {
    ink: Ink,
    palette: &'a Palette,
    highlighter: Option<&'a Highlighter>,
    width: usize,
    layout: Layout,
}

impl Page<'_> {
    fn header(&self, commit: &Commit, diffs: &[FileDiff]) -> String {
        let (ink, palette) = (self.ink, self.palette);
        let mut out = ink.bold(&commit.sha, Tone::Indexed(3));
        if !commit.refs.is_empty() {
            out += &ink.paint(&format!(" ({})", commit.refs), Tone::Faint, None);
        }
        out += &format!(
            "\n{} {}\n{}\n",
            ink.bold(&commit.author, Tone::Default),
            ink.paint(&format!("<{}>", commit.email), Tone::Faint, None),
            ink.paint(
                &format!("{} · {}", commit.date, commit.relative),
                Tone::Faint,
                None
            ),
        );
        let (subject, body) = commit
            .message
            .split_once('\n')
            .unwrap_or((&commit.message, ""));
        out += &format!("\n    {}\n", ink.bold(subject, Tone::Default));
        let body = body.trim();
        if !body.is_empty() {
            out.push('\n');
            for line in body.lines() {
                out += &format!("    {}\n", ink.paint(line, Tone::Default, None));
            }
        }
        let additions: u32 = diffs.iter().map(|diff| diff.additions).sum();
        let deletions: u32 = diffs.iter().map(|diff| diff.deletions).sum();
        out += &format!(
            "\n{} file{} {} {}\n",
            diffs.len(),
            if diffs.len() == 1 { "" } else { "s" },
            ink.paint(&format!("+{additions}"), palette.added, None),
            ink.paint(&format!("-{deletions}"), palette.deleted, None),
        );
        out
    }

    fn tint(&self, kind: LineKind) -> Tint {
        let palette = self.palette;
        match kind {
            LineKind::Addition => Tint {
                sign: '+',
                sign_color: palette.added,
                number: palette.added,
                gutter: palette.added_gutter,
                background: palette.added_background,
            },
            LineKind::Deletion => Tint {
                sign: '-',
                sign_color: palette.deleted,
                number: palette.deleted,
                gutter: palette.deleted_gutter,
                background: palette.deleted_background,
            },
            LineKind::Context | LineKind::NoNewline => Tint {
                sign: ' ',
                sign_color: Tone::Faint,
                number: Tone::Faint,
                gutter: None,
                background: None,
            },
        }
    }

    fn file(&self, out: &mut String, diff: &FileDiff) {
        let (ink, palette) = (self.ink, self.palette);
        let rule = ink.paint(&"─".repeat(self.width), Tone::Faint, None);
        let name = match (&diff.old_path, &diff.new_path) {
            (Some(old), Some(new)) if old != new => format!("{old} → {new}"),
            _ => diff.path().to_string(),
        };
        let status = match (&diff.old_path, &diff.new_path) {
            (None, Some(_)) => "  new file",
            (Some(_), None) => "  deleted",
            _ => "",
        };
        *out += &format!(
            "\n{rule}\n{}{}  {} {}\n{rule}\n",
            ink.bold(&name, Tone::Default),
            ink.paint(status, Tone::Faint, None),
            ink.paint(&format!("+{}", diff.additions), palette.added, None),
            ink.paint(&format!("-{}", diff.deletions), palette.deleted, None),
        );
        if diff.is_binary {
            *out += &format!(
                "{}\n",
                ink.paint("binary file, not shown", Tone::Faint, None)
            );
            return;
        }
        if diff.hunks.is_empty() {
            *out += &format!("{}\n", ink.paint("no content changes", Tone::Faint, None));
            return;
        }
        let largest = diff
            .hunks
            .iter()
            .map(|hunk| (hunk.old_start + hunk.old_count).max(hunk.new_start + hunk.new_count))
            .max()
            .unwrap_or(0);
        let digits = largest.to_string().len();
        for hunk in &diff.hunks {
            let spans = self.spans(diff, hunk);
            match self.layout {
                Layout::Unified => self.unified(out, hunk, &spans, digits),
                Layout::Split => self.split(out, hunk, &spans, digits),
            }
        }
    }

    /// Each side of the hunk highlighted on its own, so neither borrows the other's state.
    fn spans(&self, diff: &FileDiff, hunk: &Hunk) -> Vec<Vec<Span>> {
        let texts: Vec<String> = hunk
            .lines
            .iter()
            .map(|line| printable(&line.text))
            .collect();
        let Some(highlighter) = self.highlighter else {
            return texts
                .into_iter()
                .map(|text| vec![Span::plain(&text)])
                .collect();
        };
        let mut spans = vec![vec![]; texts.len()];
        let sides = [
            (
                diff.old_path.as_deref().unwrap_or(diff.path()),
                LineKind::Deletion,
            ),
            (diff.path(), LineKind::Addition),
        ];
        for (path, kind) in sides {
            let indices: Vec<usize> = hunk
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.kind == kind || line.kind == LineKind::Context)
                .map(|(index, _)| index)
                .collect();
            let lines: Vec<&str> = indices.iter().map(|&index| texts[index].as_str()).collect();
            for (index, highlighted) in indices.into_iter().zip(highlighter.lines(path, &lines)) {
                spans[index] = highlighted;
            }
        }
        spans
    }

    fn hunk_header(&self, out: &mut String, hunk: &Hunk, gutter: usize) {
        let label = format!("{:>gutter$} {}", "⋯", printable(hunk.section.trim()));
        let label: String = label.chars().take(self.width).collect();
        match self.palette.hunk {
            Some(background) => {
                let padding = " ".repeat(self.width.saturating_sub(label.width()));
                *out +=
                    &self
                        .ink
                        .paint(&format!("{label}{padding}"), Tone::Faint, Some(background));
            }
            None => *out += &self.ink.paint(label.trim_end(), Tone::Faint, None),
        }
        out.push('\n');
    }

    fn rows(&self, cell: &Cell, content: usize, pad: bool) -> Vec<String> {
        let (ink, tint) = (self.ink, self.tint(cell.kind));
        let pad = pad || (ink.on && tint.background.is_some());
        wrap(cell.spans, content)
            .into_iter()
            .enumerate()
            .map(|(index, chunk)| {
                let (gutter, sign) = if index == 0 {
                    (cell.gutter.clone(), tint.sign)
                } else {
                    (" ".repeat(cell.gutter.width()), ' ')
                };
                let mut row = ink.paint(&gutter, tint.number, tint.gutter);
                row += &ink.paint(&format!("{sign} "), tint.sign_color, tint.background);
                let mut used = 0;
                for span in &chunk {
                    used += span.text.width();
                    row += &if span.italic {
                        ink.italic(&span.text, span.tone, tint.background)
                    } else {
                        ink.paint(&span.text, span.tone, tint.background)
                    };
                }
                if pad {
                    let padding = " ".repeat(content.saturating_sub(used));
                    row += &ink.paint(&padding, Tone::Default, tint.background);
                }
                row
            })
            .collect()
    }

    fn unified(&self, out: &mut String, hunk: &Hunk, spans: &[Vec<Span>], digits: usize) {
        let gutter = digits * 2 + 2;
        self.hunk_header(out, hunk, gutter - 1);
        let content = self.width.saturating_sub(gutter + 2).max(10);
        for (line, spans) in hunk.lines.iter().zip(spans) {
            if line.kind == LineKind::NoNewline {
                continue;
            }
            let cell = Cell {
                gutter: format!(
                    "{:>digits$} {:>digits$} ",
                    number(line.old_number),
                    number(line.new_number)
                ),
                kind: line.kind,
                spans,
            };
            for row in self.rows(&cell, content, false) {
                *out += &row;
                out.push('\n');
            }
        }
    }

    fn split(&self, out: &mut String, hunk: &Hunk, spans: &[Vec<Span>], digits: usize) {
        self.hunk_header(out, hunk, digits);
        let side = self.width.saturating_sub(3) / 2;
        let content = side.saturating_sub(digits + 3).max(10);
        let separator = self.ink.paint(" │ ", Tone::Faint, None);
        let side_rows = |index: Option<usize>, number_of: fn(&DiffLine) -> Option<u32>, pad| {
            let Some(index) = index else {
                return (vec![], None);
            };
            let line = &hunk.lines[index];
            let cell = Cell {
                gutter: format!("{:>digits$} ", number(number_of(line))),
                kind: line.kind,
                spans: &spans[index],
            };
            (self.rows(&cell, content, pad), Some(line.kind))
        };
        for (left, right) in split_rows(hunk) {
            let (mut left, left_kind) = side_rows(left, |line| line.old_number, true);
            let (mut right, right_kind) = side_rows(right, |line| line.new_number, false);
            let height = left.len().max(right.len());
            left.resize(height, self.filler(left_kind, digits + 1, content));
            right.resize(height, self.filler(right_kind, digits + 1, content));
            for (left, right) in left.iter().zip(&right) {
                *out += &format!("{left}{separator}{}\n", right.trim_end());
            }
        }
    }

    /// The rest of a side whose line wrapped less than its neighbour's, still in its colour.
    fn filler(&self, kind: Option<LineKind>, gutter: usize, content: usize) -> String {
        let tint = self.tint(kind.unwrap_or(LineKind::Context));
        let fill = |width: usize, background| {
            self.ink
                .paint(&" ".repeat(width), Tone::Default, background)
        };
        fill(gutter, tint.gutter) + &fill(content + 2, tint.background)
    }
}

fn number(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str) -> Span {
        Span::plain(text)
    }

    #[test]
    fn wraps_spans_at_the_width_keeping_their_colours() {
        let rows = wrap(&[span("abcd"), span("ef"), span("界界")], 4);
        let texts: Vec<Vec<&str>> = rows
            .iter()
            .map(|row| row.iter().map(|span| span.text.as_str()).collect())
            .collect();
        assert_eq!(texts, [vec!["abcd"], vec!["ef", "界"], vec!["界"]]);
        assert_eq!(wrap(&[], 4).len(), 1);
    }

    #[test]
    fn pairs_deletions_with_the_additions_after_them() {
        let text = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1,4 +1,4 @@\n same\n-one\n-two\n+uno\n same\n+tail\n";
        let diff = &parse::diff(text)[0];
        assert_eq!(
            split_rows(&diff.hunks[0]),
            [
                (Some(0), Some(0)),
                (Some(1), Some(3)),
                (Some(2), None),
                (Some(4), Some(4)),
                (None, Some(5)),
            ]
        );
    }

    #[test]
    fn makes_control_characters_harmless() {
        assert_eq!(printable("\tlet\x1b[2J x\r"), "    let\u{FFFD}[2J x");
    }
}
