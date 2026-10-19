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

pub fn terminal_width() -> Option<usize> {
    for descriptor in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(descriptor, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_col > 0 {
            return Some(size.ws_col as usize);
        }
    }
    std::env::var("COLUMNS").ok()?.parse().ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A foreground: the terminal's own (plain or faint), one of its palette, or an exact colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Default,
    Faint,
    Indexed(u8),
    Exact(Rgb),
}

pub fn mix(first: Rgb, second: Rgb, amount: f32) -> Rgb {
    let blend =
        |from: u8, to: u8| (from as f32 * amount + to as f32 * (1.0 - amount)).round() as u8;
    Rgb(
        blend(first.0, second.0),
        blend(first.1, second.1),
        blend(first.2, second.2),
    )
}

/// Exact colours where the terminal says it has them (`COLORTERM`), the nearest of 256 elsewhere.
#[derive(Clone, Copy)]
pub struct Ink {
    pub on: bool,
    pub truecolor: bool,
}

impl Ink {
    pub fn stdout() -> Self {
        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        Ink {
            on: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
            truecolor: matches!(colorterm.as_str(), "truecolor" | "24bit"),
        }
    }

    fn color(self, layer: u8, color: Rgb) -> String {
        let Rgb(red, green, blue) = color;
        if self.truecolor {
            format!("{layer};2;{red};{green};{blue}")
        } else {
            format!("{layer};5;{}", nearest_of_256(color))
        }
    }

    fn tone(self, tone: Tone) -> String {
        match tone {
            Tone::Default => "39".into(),
            Tone::Faint => "39;2".into(),
            Tone::Indexed(index) => format!("38;5;{index}"),
            Tone::Exact(color) => self.color(38, color),
        }
    }

    pub fn paint(self, text: &str, tone: Tone, background: Option<Rgb>) -> String {
        self.styled(text, tone, background, "")
    }

    pub fn italic(self, text: &str, tone: Tone, background: Option<Rgb>) -> String {
        self.styled(text, tone, background, ";3")
    }

    pub fn bold(self, text: &str, tone: Tone) -> String {
        self.styled(text, tone, None, ";1")
    }

    fn styled(self, text: &str, tone: Tone, background: Option<Rgb>, extra: &str) -> String {
        if !self.on || text.is_empty() {
            return text.to_string();
        }
        let background = match background {
            Some(color) => self.color(48, color),
            None => "49".into(),
        };
        format!(
            "\x1b[0;{};{background}{extra}m{text}\x1b[0m",
            self.tone(tone)
        )
    }
}

/// Asks the terminal for its background colour (OSC 11). Device attributes (DA1) go right
/// behind it because every terminal answers those, so one that ignores OSC 11 costs no wait.
pub fn background() -> Option<Rgb> {
    use std::io::Read;
    use std::os::fd::AsRawFd;

    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;
    let descriptor = tty.as_raw_fd();
    let mut original: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(descriptor, &mut original) } != 0 {
        return None;
    }
    let mut raw = original;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 1;
    unsafe { libc::tcsetattr(descriptor, libc::TCSANOW, &raw) };
    let mut answer = Vec::new();
    if (&tty).write_all(b"\x1b]11;?\x1b\\\x1b[c").is_ok() {
        let started = Instant::now();
        let mut buffer = [0u8; 128];
        while started.elapsed() < Duration::from_millis(300) {
            match (&tty).read(&mut buffer) {
                Ok(read) => answer.extend_from_slice(&buffer[..read]),
                Err(_) => break,
            }
            let text = String::from_utf8_lossy(&answer);
            if text
                .split("\x1b[?")
                .nth(1)
                .is_some_and(|attributes| attributes.contains('c'))
            {
                break;
            }
        }
    }
    unsafe { libc::tcsetattr(descriptor, libc::TCSANOW, &original) };
    parse_background(&String::from_utf8_lossy(&answer))
}

/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB` with one to four hex digits a channel.
fn parse_background(answer: &str) -> Option<Rgb> {
    let rest = answer.split("rgb:").nth(1)?;
    let channels: Vec<u8> = rest
        .split(['/', '\x07', '\x1b'])
        .take(3)
        .map(|digits| {
            let value = u32::from_str_radix(digits, 16).ok()?;
            let largest = (1u32 << (4 * digits.len().clamp(1, 4))) - 1;
            Some((value * 255 / largest) as u8)
        })
        .collect::<Option<_>>()?;
    match channels[..] {
        [red, green, blue] => Some(Rgb(red, green, blue)),
        _ => None,
    }
}

fn nearest_of_256(Rgb(red, green, blue): Rgb) -> u8 {
    let level = |value: u8| -> u8 {
        if value < 48 {
            0
        } else if value < 115 {
            1
        } else {
            (value - 35) / 40
        }
    };
    let value_of = |level: u8| -> u8 {
        if level == 0 {
            0
        } else {
            55 + level * 40
        }
    };
    let distance = |candidate: [u8; 3]| -> u32 {
        candidate
            .iter()
            .zip([red, green, blue])
            .map(|(&from, to)| (from as i32 - to as i32).pow(2) as u32)
            .sum()
    };
    let levels = [level(red), level(green), level(blue)];
    let cube = 16 + 36 * levels[0] + 6 * levels[1] + levels[2];
    let cube_distance = distance(levels.map(value_of));
    let average = (red as u32 + green as u32 + blue as u32) / 3;
    let gray = (average.saturating_sub(3) / 10).min(23) as u8;
    let gray_value = 8 + gray * 10;
    if distance([gray_value; 3]) < cube_distance {
        232 + gray
    } else {
        cube
    }
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

    #[test]
    fn reads_the_background_the_terminal_reports() {
        assert_eq!(
            parse_background("\x1b]11;rgb:1212/1313/1717\x1b\\\x1b[?62;22c"),
            Some(Rgb(0x12, 0x13, 0x17))
        );
        assert_eq!(
            parse_background("\x1b]11;rgb:ff/ff/ff\x07"),
            Some(Rgb(255, 255, 255))
        );
        assert_eq!(parse_background("\x1b[?1;2c"), None);
        assert_eq!(mix(Rgb(255, 0, 0), Rgb(0, 0, 0), 0.5), Rgb(128, 0, 0));
    }

    #[test]
    fn picks_the_nearest_of_256_colours() {
        assert_eq!(nearest_of_256(Rgb(0, 0, 0)), 16);
        assert_eq!(nearest_of_256(Rgb(255, 255, 255)), 231);
        assert_eq!(nearest_of_256(Rgb(0x12, 0x13, 0x17)), 233);
        assert_eq!(nearest_of_256(Rgb(0x6c, 0xb6, 0xff)), 75);
    }
}
