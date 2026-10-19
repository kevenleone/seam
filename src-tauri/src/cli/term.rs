use std::future::Future;
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

const BLUE: &str = "38;5;75";
const ORANGE: &str = "38;5;209";
const PURPLE: &str = "38;5;141";
const DIM: &str = "2";
const BOLD: &str = "1";

#[derive(Clone, Copy)]
pub struct Paint {
    on: bool,
}

impl Paint {
    pub fn stdout() -> Self {
        Paint {
            on: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    pub fn stderr() -> Self {
        Paint {
            on: std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn added(self, text: &str) -> String {
        self.wrap(BLUE, text)
    }

    pub fn deleted(self, text: &str) -> String {
        self.wrap(ORANGE, text)
    }

    pub fn accent(self, text: &str) -> String {
        self.wrap(PURPLE, text)
    }

    pub fn heading(self, text: &str) -> String {
        self.wrap(&format!("{BOLD};{PURPLE}"), text)
    }

    pub fn bold(self, text: &str) -> String {
        self.wrap(BOLD, text)
    }

    pub fn dim(self, text: &str) -> String {
        self.wrap(DIM, text)
    }

    pub fn good(self, ok: bool) -> String {
        if ok {
            self.added("✓")
        } else {
            self.deleted("✗")
        }
    }
}

pub fn seconds(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

/// Runs `work` behind a ticking `label…` line on stderr, then leaves `label… done in 41s`.
pub async fn progress<T>(label: &str, work: impl Future<Output = T>) -> T {
    let paint = Paint::stderr();
    let live = std::io::stderr().is_terminal();
    let started = Instant::now();
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tokio::pin!(work);
    let result = loop {
        tokio::select! {
            result = &mut work => break result,
            _ = tick.tick(), if live => {
                eprint!("\r\x1b[2K{label}… {}", paint.dim(&seconds(started.elapsed())));
                let _ = std::io::stderr().flush();
            }
        }
    };
    if live {
        eprint!("\r\x1b[2K");
    }
    eprintln!("{label}… done in {}", seconds(started.elapsed()));
    result
}

/// Wraps prose to the terminal, or 88 columns when it isn't one.
pub fn wrap(text: &str, indent: &str) -> String {
    let width = terminal_width().unwrap_or(88).clamp(40, 100) - indent.len();
    let mut out = Vec::new();
    for paragraph in text.split("\n\n") {
        let mut line = String::new();
        let mut lines = Vec::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                lines.push(format!("{indent}{line}"));
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(format!("{indent}{line}"));
        }
        out.push(lines.join("\n"));
    }
    out.join("\n\n")
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS").ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_paragraphs_without_breaking_words() {
        let text = format!("{}\n\n{}", "word ".repeat(30).trim(), "short");
        let wrapped = wrap(&text, "  ");
        let lines: Vec<&str> = wrapped.lines().collect();
        assert!(lines.iter().all(|line| line.len() <= 100));
        assert!(lines
            .iter()
            .filter(|line| !line.is_empty())
            .all(|line| line.starts_with("  ")));
        assert_eq!(*lines.last().unwrap(), "  short");
        assert_eq!(seconds(Duration::from_secs(41)), "41s");
        assert_eq!(seconds(Duration::from_secs(125)), "2m 05s");
    }
}
