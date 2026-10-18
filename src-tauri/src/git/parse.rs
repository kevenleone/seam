//! Parsers for git's machine-readable output.

use super::models::*;

/// Field and record separators used in `--format` strings. Unit/record separators never
/// appear in commit metadata, unlike newlines or NUL inside bodies.
const FIELD: char = '\u{1F}';
const RECORD: char = '\u{1E}';

// MARK: Status

/// Parses `git status --porcelain=v2 -z --branch`.
pub fn status(data: &[u8]) -> RepoStatus {
    let text = String::from_utf8_lossy(data);
    let fields: Vec<&str> = text.split('\0').collect();
    let mut status = RepoStatus::default();
    let mut i = 0;
    while i < fields.len() {
        let entry = fields[i];
        i += 1;
        let Some(kind) = entry.chars().next() else {
            continue;
        };
        match kind {
            '#' => header(entry, &mut status.branch),
            '1' => {
                // 1 XY sub mH mI mW hH hI path
                let parts: Vec<&str> = entry.splitn(9, ' ').collect();
                if parts.len() == 9 {
                    add(parts[1], parts[8], None, &mut status);
                }
            }
            '2' => {
                // 2 XY sub mH mI mW hH hI Xscore path, then the original path as the next field
                let parts: Vec<&str> = entry.splitn(10, ' ').collect();
                let original = fields.get(i).map(|s| s.to_string());
                i += 1;
                if parts.len() == 10 {
                    add(parts[1], parts[9], original, &mut status);
                }
            }
            'u' => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let parts: Vec<&str> = entry.splitn(11, ' ').collect();
                if parts.len() == 11 {
                    status.conflicted.push(change(
                        parts[10],
                        None,
                        ChangeKind::Conflicted,
                        ChangeArea::Conflicted,
                    ));
                }
            }
            '?' => status.untracked.push(change(
                &entry[2..],
                None,
                ChangeKind::Untracked,
                ChangeArea::Untracked,
            )),
            _ => {}
        }
    }
    status
}

fn change(path: &str, original: Option<String>, kind: ChangeKind, area: ChangeArea) -> FileChange {
    FileChange {
        path: path.to_string(),
        original_path: original,
        kind,
        area,
    }
}

fn header(line: &str, branch: &mut BranchStatus) {
    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() < 3 {
        return;
    }
    let value = parts[2..].join(" ");
    match parts[1] {
        "branch.oid" => branch.oid = (value != "(initial)").then_some(value),
        "branch.head" => branch.head = (value != "(detached)").then_some(value),
        "branch.upstream" => branch.upstream = Some(value),
        "branch.ab" => {
            for token in &parts[2..] {
                if let Some(n) = token.strip_prefix('+') {
                    branch.ahead = n.parse().unwrap_or(0);
                }
                if let Some(n) = token.strip_prefix('-') {
                    branch.behind = n.parse().unwrap_or(0);
                }
            }
        }
        _ => {}
    }
}

fn add(xy: &str, path: &str, original: Option<String>, status: &mut RepoStatus) {
    let chars: Vec<char> = xy.chars().collect();
    if chars.len() != 2 {
        return;
    }
    if let Some(kind) = kind_for(chars[0]) {
        status
            .staged
            .push(change(path, original.clone(), kind, ChangeArea::Staged));
    }
    if let Some(kind) = kind_for(chars[1]) {
        status
            .unstaged
            .push(change(path, original, kind, ChangeArea::Unstaged));
    }
}

fn kind_for(c: char) -> Option<ChangeKind> {
    Some(match c {
        'A' => ChangeKind::Added,
        'M' => ChangeKind::Modified,
        'D' => ChangeKind::Deleted,
        'R' => ChangeKind::Renamed,
        'C' => ChangeKind::Copied,
        'T' => ChangeKind::TypeChanged,
        _ => return None,
    })
}

// MARK: Diff

struct HunkMeta {
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
    section: String,
}

/// Parses unified diff output from `git diff --no-color`.
pub fn diff(text: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }

    let mut file: Option<FileBuilder> = None;
    for line in lines {
        if line.starts_with("diff --git ") {
            if let Some(f) = file.take() {
                files.push(f.finish());
            }
            file = Some(FileBuilder::new(line));
            continue;
        }
        if let Some(f) = file.as_mut() {
            f.push(line);
        }
    }
    if let Some(f) = file.take() {
        files.push(f.finish());
    }
    files
}

struct FileBuilder {
    header: Vec<String>,
    hunks: Vec<Hunk>,
    old_path: Option<String>,
    new_path: Option<String>,
    is_binary: bool,
    meta: Option<HunkMeta>,
    lines: Vec<DiffLine>,
    old_line: u32,
    new_line: u32,
}

impl FileBuilder {
    fn new(first: &str) -> Self {
        let (old_path, new_path) = paths_from_diff_git(first);
        FileBuilder {
            header: vec![first.to_string()],
            hunks: Vec::new(),
            old_path,
            new_path,
            is_binary: false,
            meta: None,
            lines: Vec::new(),
            old_line: 0,
            new_line: 0,
        }
    }

    fn flush_hunk(&mut self) {
        if let Some(m) = self.meta.take() {
            self.hunks.push(Hunk {
                id: self.hunks.len(),
                old_start: m.old_start,
                old_count: m.old_count,
                new_start: m.new_start,
                new_count: m.new_count,
                section: m.section,
                lines: std::mem::take(&mut self.lines),
            });
        }
    }

    fn push(&mut self, line: &str) {
        if self.meta.is_none() || line.starts_with("@@") {
            if let Some(meta) = hunk_header(line) {
                self.flush_hunk();
                self.old_line = meta.old_start;
                self.new_line = meta.new_start;
                self.meta = Some(meta);
                return;
            }
            // Still in the file header.
            self.header.push(line.to_string());
            if let Some(p) = line.strip_prefix("--- ") {
                self.old_path = (line != "--- /dev/null").then(|| strip_prefix(p));
            } else if let Some(p) = line.strip_prefix("+++ ") {
                self.new_path = (line != "+++ /dev/null").then(|| strip_prefix(p));
            } else if line.starts_with("new file mode") {
                self.old_path = None;
            } else if line.starts_with("deleted file mode") {
                self.new_path = None;
            } else if let Some(p) = line.strip_prefix("rename from ") {
                self.old_path = Some(p.to_string());
            } else if let Some(p) = line.strip_prefix("rename to ") {
                self.new_path = Some(p.to_string());
            } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
                self.is_binary = true;
            }
            return;
        }

        let id = self.lines.len();
        let (first, body) = match line.chars().next() {
            Some(c) => (c, &line[c.len_utf8()..]),
            // An empty line inside a hunk is a context line whose leading space was trimmed.
            None => (' ', ""),
        };
        let (kind, old, new) = match first {
            '+' => {
                self.new_line += 1;
                (LineKind::Addition, None, Some(self.new_line - 1))
            }
            '-' => {
                self.old_line += 1;
                (LineKind::Deletion, Some(self.old_line - 1), None)
            }
            '\\' => (LineKind::NoNewline, None, None),
            _ => {
                self.old_line += 1;
                self.new_line += 1;
                (
                    LineKind::Context,
                    Some(self.old_line - 1),
                    Some(self.new_line - 1),
                )
            }
        };
        let text = if kind == LineKind::NoNewline {
            line.to_string()
        } else {
            body.to_string()
        };
        self.lines.push(DiffLine {
            id,
            kind,
            text,
            old_number: old,
            new_number: new,
        });
    }

    fn finish(mut self) -> FileDiff {
        self.flush_hunk();
        let count = |k: LineKind| {
            self.hunks
                .iter()
                .flat_map(|h| &h.lines)
                .filter(|l| l.kind == k)
                .count() as u32
        };
        let additions = count(LineKind::Addition);
        let deletions = count(LineKind::Deletion);
        FileDiff {
            old_path: self.old_path,
            new_path: self.new_path,
            header_lines: self.header,
            hunks: self.hunks,
            is_binary: self.is_binary,
            additions,
            deletions,
        }
    }
}

fn hunk_header(line: &str) -> Option<HunkMeta> {
    // @@ -a,b +c,d @@ section
    let rest = line.strip_prefix("@@ ")?;
    let end = rest.find(" @@")?;
    let ranges: Vec<&str> = rest[..end].split(' ').collect();
    if ranges.len() != 2 {
        return None;
    }
    let range = |s: &str, sign: char| -> Option<(u32, u32)> {
        let s = s.strip_prefix(sign)?;
        let mut parts = s.split(',');
        let start = parts.next()?.parse().ok()?;
        let count = parts.next().map(|c| c.parse().unwrap_or(1)).unwrap_or(1);
        Some((start, count))
    };
    let (old_start, old_count) = range(ranges[0], '-')?;
    let (new_start, new_count) = range(ranges[1], '+')?;
    Some(HunkMeta {
        old_start,
        old_count,
        new_start,
        new_count,
        section: rest[end + 3..].trim().to_string(),
    })
}

fn strip_prefix(path: &str) -> String {
    let mut p = path.trim_end_matches('\t');
    if p.len() >= 2 && p.starts_with('"') && p.ends_with('"') {
        p = &p[1..p.len() - 1];
    }
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
        .to_string()
}

fn paths_from_diff_git(line: &str) -> (Option<String>, Option<String>) {
    // diff --git a/path b/path is ambiguous with spaces; refined later by ---/+++ or rename lines.
    let rest = &line["diff --git ".len()..];
    if let (Some(stripped), Some(i)) = (rest.strip_prefix("a/"), rest.rfind(" b/")) {
        let old = stripped[..i - 2].to_string();
        let new = rest[i + 3..].to_string();
        return (Some(old), Some(new));
    }
    (None, None)
}

// MARK: Log, refs, stashes

pub fn log_format() -> String {
    ["%H", "%P", "%an", "%ae", "%at", "%s", "%b", "%D"].join("%x1F") + "%x1E"
}

pub fn log(text: &str) -> Vec<Commit> {
    text.split(RECORD)
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            if record.is_empty() {
                return None;
            }
            let f: Vec<&str> = record.split(FIELD).collect();
            if f.len() < 8 {
                return None;
            }
            Some(Commit {
                sha: f[0].to_string(),
                parents: f[1]
                    .split(' ')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
                author_name: f[2].to_string(),
                author_email: f[3].to_string(),
                date: f[4].parse().unwrap_or(0),
                subject: f[5].to_string(),
                body: f[6].trim().to_string(),
                refs: f[7]
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            })
        })
        .collect()
}

pub fn branch_format() -> String {
    [
        "%(refname)",
        "%(refname:short)",
        "%(objectname)",
        "%(HEAD)",
        "%(upstream:short)",
        "%(upstream:track,nobracket)",
        "%(worktreepath)",
        "%(committerdate:unix)",
        "%(authorname)",
        "%(contents:subject)",
    ]
    .join("%1F")
}

pub fn branches(text: &str) -> Vec<Branch> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(FIELD).collect();
            if f.len() < 6 || f[0].ends_with("/HEAD") {
                // Skip symbolic refs like origin/HEAD.
                return None;
            }
            let (mut ahead, mut behind) = (0, 0);
            for part in f[5].split(',') {
                let tokens: Vec<&str> = part.trim().split(' ').collect();
                if tokens.len() == 2 {
                    let n = tokens[1].parse().unwrap_or(0);
                    match tokens[0] {
                        "ahead" => ahead = n,
                        "behind" => behind = n,
                        _ => {}
                    }
                }
            }
            Some(Branch {
                name: f[1].to_string(),
                full_name: f[0].to_string(),
                sha: f[2].to_string(),
                is_remote: f[0].starts_with("refs/remotes/"),
                is_current: f[3] == "*",
                upstream: (!f[4].is_empty()).then(|| f[4].to_string()),
                ahead,
                behind,
                upstream_gone: f[5] == "gone",
                worktree: f
                    .get(6)
                    .filter(|path| !path.is_empty() && f[3] != "*")
                    .map(|path| path.to_string()),
                date: f.get(7).and_then(|date| date.parse().ok()).unwrap_or(0),
                author: f.get(8).unwrap_or(&"").to_string(),
                subject: f.get(9).unwrap_or(&"").to_string(),
            })
        })
        .collect()
}

/// `git worktree list --porcelain`: blank-line separated records, the main worktree first.
pub fn worktrees(text: &str) -> Vec<Worktree> {
    text.split("\n\n")
        .filter_map(|record| {
            let mut worktree = Worktree {
                path: String::new(),
                branch: None,
                sha: None,
                bare: false,
            };
            for line in record.lines() {
                let (key, value) = line.split_once(' ').unwrap_or((line, ""));
                match key {
                    "worktree" => worktree.path = value.to_string(),
                    "HEAD" => worktree.sha = Some(value.to_string()),
                    "branch" => {
                        worktree.branch = Some(
                            value
                                .strip_prefix("refs/heads/")
                                .unwrap_or(value)
                                .to_string(),
                        )
                    }
                    "bare" => worktree.bare = true,
                    _ => {}
                }
            }
            (!worktree.path.is_empty()).then_some(worktree)
        })
        .collect()
}

pub fn tag_format() -> String {
    ["%(refname:short)", "%(objectname)", "%(*objectname)"].join("%1F")
}

pub fn tags(text: &str) -> Vec<Tag> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split(FIELD).collect();
            // Annotated tags point at a tag object; %(*objectname) is the commit it peels to.
            (f.len() >= 3).then(|| Tag {
                name: f[0].to_string(),
                sha: if f[2].is_empty() { f[1] } else { f[2] }.to_string(),
            })
        })
        .collect()
}

pub fn stash_format() -> String {
    ["%H", "%gd", "%at", "%gs"].join("%x1F") + "%x1E"
}

pub fn stashes(text: &str) -> Vec<Stash> {
    text.split(RECORD)
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            let f: Vec<&str> = record.split(FIELD).collect();
            if f.len() < 4 {
                return None;
            }
            let index = f[1]
                .split('{')
                .nth(1)
                .and_then(|s| s.trim_end_matches('}').parse().ok())
                .unwrap_or(0);
            Some(Stash {
                index,
                sha: f[0].to_string(),
                message: clean_stash_message(f[3]),
                date: f[2].parse().unwrap_or(0),
            })
        })
        .collect()
}

/// "On main: message" / "WIP on main: abc123 subject" → the part after the colon.
pub fn clean_stash_message(message: &str) -> String {
    message
        .split_once(": ")
        .map(|(_, m)| m.to_string())
        .unwrap_or_else(|| message.to_string())
}

/// Parses `git show --numstat -z --format=` output.
pub fn numstat(data: &[u8]) -> Vec<CommitFileStat> {
    let text = String::from_utf8_lossy(data);
    let fields: Vec<&str> = text.split('\0').collect();
    let mut stats = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let entry = fields[i].trim_matches('\n');
        i += 1;
        let parts: Vec<&str> = entry.splitn(3, '\t').collect();
        if parts.len() != 3 {
            continue;
        }
        let mut path = parts[2].to_string();
        if path.is_empty() {
            // Rename: old and new path follow as separate fields.
            path = fields.get(i + 1).unwrap_or(&"").to_string();
            i += 2;
        }
        stats.push(CommitFileStat {
            path,
            additions: parts[0].parse().ok(),
            deletions: parts[1].parse().ok(),
        });
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_v2() {
        let raw = [
            "# branch.oid 1234567890abcdef",
            "# branch.head feature/x",
            "# branch.upstream origin/feature/x",
            "# branch.ab +3 -1",
            "1 M. N... 100644 100644 100644 aaa bbb src/a.swift",
            "1 .M N... 100644 100644 100644 aaa bbb src/b.swift",
            "1 MM N... 100644 100644 100644 aaa bbb c d.txt",
            "2 R. N... 100644 100644 100644 aaa bbb R100 new.txt",
            "old.txt",
            "u UU N... 100644 100644 100644 100644 aaa bbb ccc conflict.txt",
            "? untracked file.txt",
            "",
        ]
        .join("\0");
        let s = status(raw.as_bytes());
        assert_eq!(s.branch.head.as_deref(), Some("feature/x"));
        assert_eq!(s.branch.upstream.as_deref(), Some("origin/feature/x"));
        assert_eq!((s.branch.ahead, s.branch.behind), (3, 1));
        let paths = |v: &[FileChange]| v.iter().map(|c| c.path.clone()).collect::<Vec<_>>();
        assert_eq!(paths(&s.staged), ["src/a.swift", "c d.txt", "new.txt"]);
        assert_eq!(s.staged[2].original_path.as_deref(), Some("old.txt"));
        assert_eq!(s.staged[2].kind, ChangeKind::Renamed);
        assert_eq!(paths(&s.unstaged), ["src/b.swift", "c d.txt"]);
        assert_eq!(paths(&s.conflicted), ["conflict.txt"]);
        assert_eq!(paths(&s.untracked), ["untracked file.txt"]);
    }

    #[test]
    fn detached_and_initial() {
        let s = status(b"# branch.oid (initial)\0# branch.head (detached)\0");
        assert_eq!(s.branch.oid, None);
        assert_eq!(s.branch.head, None);
    }

    #[test]
    fn unified_diff() {
        let text = "diff --git a/src/rate.ts b/src/rate.ts\nindex 111..222 100644\n--- a/src/rate.ts\n+++ b/src/rate.ts\n@@ -12,4 +12,5 @@ export function rateLimit(opts) {\n   const store = opts.store;\n-  const max = opts.max || 100;\n+  const max = opts.max ?? 100;\n+  const keyBy = opts.keyBy;\n   return next;\n@@ -40 +41 @@\n-a\n+b\n\\ No newline at end of file\ndiff --git a/new.txt b/new.txt\nnew file mode 100644\nindex 0000000..333\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+one\n+two\n";
        let files = diff(text);
        assert_eq!(files.len(), 2);
        let f = &files[0];
        assert_eq!(f.path(), "src/rate.ts");
        assert_eq!(f.hunks.len(), 2);
        assert_eq!(f.hunks[0].section, "export function rateLimit(opts) {");
        let kinds: Vec<LineKind> = f.hunks[0].lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LineKind::Context,
                LineKind::Deletion,
                LineKind::Addition,
                LineKind::Addition,
                LineKind::Context
            ]
        );
        assert_eq!(f.hunks[0].lines[2].new_number, Some(13));
        assert_eq!(f.hunks[0].lines[4].old_number, Some(14));
        assert_eq!(f.hunks[0].lines[4].new_number, Some(15));
        assert_eq!(f.hunks[1].old_count, 1);
        assert_eq!(f.hunks[1].lines.last().unwrap().kind, LineKind::NoNewline);
        assert_eq!((f.additions, f.deletions), (3, 2));
        assert_eq!(files[1].old_path, None);
        assert_eq!(files[1].new_path.as_deref(), Some("new.txt"));
    }

    #[test]
    fn binary_diff() {
        let files = diff("diff --git a/img.png b/img.png\nindex 1..2 100644\nBinary files a/img.png and b/img.png differ\n");
        assert!(files[0].is_binary);
        assert_eq!(files[0].path(), "img.png");
    }

    #[test]
    fn log_records() {
        let (f, r) = (FIELD, RECORD);
        let text = format!(
            "abc{f}p1 p2{f}Ana{f}ana@x.io{f}1700000000{f}Merge it{f}Body line\n{f}HEAD -> main, tag: v1.0{r}\ndef{f}{f}Bo{f}bo@x.io{f}1600000000{f}Root{f}{f}{r}\n"
        );
        let commits = log(&text);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].parents, ["p1", "p2"]);
        assert_eq!(commits[0].body, "Body line");
        assert_eq!(commits[0].refs, ["HEAD -> main", "tag: v1.0"]);
        assert!(commits[1].parents.is_empty());
    }

    #[test]
    fn worktree_list() {
        let text = "worktree /src/seam\nHEAD aaa\nbranch refs/heads/main\n\n\
                    worktree /src/seam-side\nHEAD bbb\nbranch refs/heads/feat/side\n\n\
                    worktree /src/seam-old\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n";
        let list = worktrees(text);
        assert_eq!(
            list.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(),
            ["/src/seam", "/src/seam-side", "/src/seam-old"]
        );
        assert_eq!(list[1].branch.as_deref(), Some("feat/side"));
        assert_eq!(list[2].branch, None);
        assert_eq!(list[2].sha.as_deref(), Some("ccc"));
        assert!(!list[0].bare);
        assert!(worktrees("worktree /src/bare.git\nbare\n")[0].bare);
    }

    #[test]
    fn branch_refs() {
        let f = FIELD;
        let text = [
            format!("refs/heads/main{f}main{f}aaa{f}*{f}origin/main{f}ahead 2, behind 1"),
            format!("refs/heads/old{f}old{f}bbb{f} {f}origin/old{f}gone{f}/tmp/old-tree{f}1700000000{f}Ana{f}fix: a thing"),
            format!("refs/remotes/origin/HEAD{f}origin{f}aaa{f} {f}{f}"),
            format!("refs/remotes/origin/main{f}origin/main{f}ccc{f} {f}{f}"),
        ]
        .join("\n");
        let b = branches(&text);
        assert_eq!(
            b.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            ["main", "old", "origin/main"]
        );
        assert!(b[0].is_current && b[0].ahead == 2 && b[0].behind == 1);
        assert!(b[1].upstream_gone);
        assert_eq!(b[1].worktree.as_deref(), Some("/tmp/old-tree"));
        assert_eq!(
            (b[1].date, b[1].author.as_str(), b[1].subject.as_str()),
            (1700000000, "Ana", "fix: a thing")
        );
        assert!(b[2].is_remote && b[2].worktree.is_none());
    }

    #[test]
    fn numstat_with_rename() {
        let raw = "\n3\t1\tsrc/a.ts\0-\t-\timg.png\0\x30\t0\t\0old.ts\0new.ts\0";
        let stats = numstat(raw.as_bytes());
        assert_eq!(
            stats.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            ["src/a.ts", "img.png", "new.ts"]
        );
        assert_eq!(stats[1].additions, None);
    }

    #[test]
    fn stash_messages() {
        assert_eq!(
            clean_stash_message("On main: try sliding window"),
            "try sliding window"
        );
        assert_eq!(
            clean_stash_message("WIP on main: abc123 subject"),
            "abc123 subject"
        );
    }
}
