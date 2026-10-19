use std::path::Path;

use crate::git::process::{git, RunOptions};
use crate::git::Result;

pub const TOOL: &str = "seam";

/// `git config` with `--global`, or `--file <path>` in tests.
pub async fn register(location: &[&str], program: &str, default: bool) -> Result<()> {
    let command = format!("{program} --difftool \"$LOCAL\" \"$REMOTE\" \"$MERGED\"");
    set(location, &format!("difftool.{TOOL}.cmd"), &command).await?;
    set(location, &format!("difftool.{TOOL}.trustExitCode"), "true").await?;
    if default {
        set(location, "diff.tool", TOOL).await?;
    }
    Ok(())
}

/// Removes exactly what `register` added, and `diff.tool` only while it still names Seam.
pub async fn unregister(location: &[&str]) -> Result<()> {
    let section = format!("difftool.{TOOL}");
    config(location, &["--remove-section", &section], true).await?;
    if get(location, "diff.tool").await.as_deref() == Some(TOOL) {
        config(location, &["--unset", "diff.tool"], true).await?;
    }
    Ok(())
}

pub async fn get(location: &[&str], key: &str) -> Option<String> {
    let output = config(location, &["--get", key], true).await.ok()?;
    let value = output.trim().to_string();
    (!value.is_empty()).then_some(value)
}

async fn set(location: &[&str], key: &str, value: &str) -> Result<()> {
    config(location, &[key, value], false).await.map(|_| ())
}

async fn config(location: &[&str], args: &[&str], allow_failure: bool) -> Result<String> {
    let mut all = vec!["config"];
    all.extend(location);
    all.extend(args);
    let output = git(
        &std::env::temp_dir(),
        &all,
        RunOptions {
            allow_failure,
            ..Default::default()
        },
    )
    .await?;
    Ok(output.text())
}

/// The two sides git hands a difftool, as one patch named by the repository's own paths.
pub async fn patch(cwd: &Path, left: &str, right: &str, merged: Option<&str>) -> Result<String> {
    let output = git(
        cwd,
        &[
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "-M",
            "--",
            left,
            right,
        ],
        RunOptions {
            allow_failure: true,
            ..Default::default()
        },
    )
    .await?;
    // `--no-index` exits 1 when the sides differ.
    if output.code > 1 {
        return Err(crate::git::GitError::new(output.stderr.trim().to_string()));
    }
    let both_folders = Path::new(left).is_dir() && Path::new(right).is_dir();
    Ok(relabel(&output.text(), left, right, merged, both_folders))
}

/// `git diff --no-index` names files by the paths it was given, less a leading `/`:
/// temporary copies under `--dir-diff`, or a temporary file and the real one otherwise.
pub fn relabel(
    text: &str,
    left: &str,
    right: &str,
    merged: Option<&str>,
    both_folders: bool,
) -> String {
    let sides = [left.trim_start_matches('/'), right.trim_start_matches('/')];
    let name = merged.unwrap_or(sides[1]);
    let rename = |path: &str| -> String {
        for side in sides {
            if both_folders {
                if let Some(inner) = path.strip_prefix(&format!("{side}/")) {
                    return inner.to_string();
                }
            } else if path == side {
                return name.to_string();
            }
        }
        path.to_string()
    };
    let header = |line: &str| -> Option<String> {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let (old, new) = rest.split_once(" b/")?;
            let old = old.strip_prefix("a/")?;
            return Some(format!("diff --git a/{} b/{}", rename(old), rename(new)));
        }
        for prefix in ["--- a/", "+++ b/"] {
            if let Some(path) = line.strip_prefix(prefix) {
                return Some(format!("{prefix}{}", rename(path)));
            }
        }
        for prefix in ["rename from ", "rename to ", "copy from ", "copy to "] {
            if let Some(path) = line.strip_prefix(prefix) {
                return Some(format!("{prefix}{}", rename(path)));
            }
        }
        None
    };
    let mut in_hunk = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, end) = match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        if body.starts_with("diff --git ") {
            in_hunk = false;
        } else if body.starts_with("@@") {
            in_hunk = true;
        }
        match (!in_hunk).then(|| header(body)).flatten() {
            Some(renamed) => {
                out.push_str(&renamed);
                out.push_str(end);
            }
            None => out.push_str(line),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_folder_diffs_by_the_paths_inside_them() {
        let text = "diff --git a/tmp/x/left/src/a.ts b/tmp/x/right/src/a.ts\n\
                    index 1..2 100644\n\
                    --- a/tmp/x/left/src/a.ts\n\
                    +++ b/tmp/x/right/src/a.ts\n\
                    @@ -1 +1 @@\n\
                    --- a/tmp/x/left/kept.ts\n\
                    +b\n\
                    diff --git a/tmp/x/right/new.ts b/tmp/x/right/new.ts\n\
                    new file mode 100644\n\
                    --- /dev/null\n\
                    +++ b/tmp/x/right/new.ts\n";
        let out = relabel(text, "/tmp/x/left", "/tmp/x/right", None, true);
        assert!(out.starts_with("diff --git a/src/a.ts b/src/a.ts\n"));
        assert!(out.contains("--- a/src/a.ts\n+++ b/src/a.ts\n"));
        assert!(
            out.contains("--- a/tmp/x/left/kept.ts\n"),
            "hunk lines stay as they are"
        );
        assert!(out.contains("diff --git a/new.ts b/new.ts\n"));
        assert!(out.contains("--- /dev/null\n+++ b/new.ts\n"));
    }

    #[test]
    fn names_a_single_file_diff_by_the_merged_path() {
        let text = "diff --git a/var/T/abc_a.ts b/src/a.ts\n--- a/var/T/abc_a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-a\n+b\n";
        let out = relabel(text, "/var/T/abc_a.ts", "src/a.ts", Some("src/a.ts"), false);
        assert_eq!(
            out,
            "diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-a\n+b\n"
        );
    }

    #[tokio::test]
    async fn registers_and_removes_only_its_own_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("gitconfig");
        std::fs::write(&file, "[user]\n\tname = Someone\n").unwrap();
        let file = file.to_string_lossy().into_owned();
        let location = ["--file", file.as_str()];

        register(&location, "/usr/local/bin/seam", true)
            .await
            .unwrap();
        let command = get(&location, "difftool.seam.cmd").await.unwrap();
        assert_eq!(
            command,
            r#"/usr/local/bin/seam --difftool "$LOCAL" "$REMOTE" "$MERGED""#
        );
        assert_eq!(get(&location, "diff.tool").await.as_deref(), Some("seam"));

        unregister(&location).await.unwrap();
        assert_eq!(get(&location, "difftool.seam.cmd").await, None);
        assert_eq!(get(&location, "diff.tool").await, None);
        assert_eq!(
            get(&location, "user.name").await.as_deref(),
            Some("Someone")
        );

        set(&location, "diff.tool", "vimdiff").await.unwrap();
        unregister(&location).await.unwrap();
        assert_eq!(
            get(&location, "diff.tool").await.as_deref(),
            Some("vimdiff")
        );
    }
}
