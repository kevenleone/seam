use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};

use super::term::{mix, Rgb, Tone};

/// Seam's blue and orange; changed lines are tinted by blending them into the terminal's own
/// background, so they sit on any theme. Without that background there are no tints at all.
pub struct Palette {
    pub added: Tone,
    pub deleted: Tone,
    pub added_background: Option<Rgb>,
    pub deleted_background: Option<Rgb>,
    pub added_gutter: Option<Rgb>,
    pub deleted_gutter: Option<Rgb>,
    pub hunk: Option<Rgb>,
}

const fn hex(value: u32) -> Rgb {
    Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

impl Palette {
    pub fn new(background: Option<Rgb>, truecolor: bool) -> Self {
        let dark = background.is_none_or(|Rgb(red, green, blue)| {
            0.2126 * red as f32 + 0.7152 * green as f32 + 0.0722 * (blue as f32) < 128.0
        });
        let (added, deleted, indexed) = if dark {
            (hex(0x6cb6ff), hex(0xf0a060), (75, 209))
        } else {
            (hex(0x0b5cad), hex(0xa3500f), (25, 130))
        };
        let tinted = background.filter(|_| truecolor);
        let tint =
            |color: Rgb, amount: f32| tinted.map(|background| mix(color, background, amount));
        let (line, gutter) = if dark { (0.13, 0.22) } else { (0.1, 0.18) };
        let contrast = if dark { hex(0xffffff) } else { hex(0x000000) };
        let tone = |exact: Rgb, index: u8| {
            if truecolor {
                Tone::Exact(exact)
            } else {
                Tone::Indexed(index)
            }
        };
        Palette {
            added: tone(added, indexed.0),
            deleted: tone(deleted, indexed.1),
            added_background: tint(added, line),
            deleted_background: tint(deleted, line),
            added_gutter: tint(added, gutter),
            deleted_gutter: tint(deleted, gutter),
            hunk: tint(contrast, 0.05),
        }
    }
}

const ALPHA_INDEXED: u8 = 0;
const ALPHA_DEFAULT: u8 = 1;
const ALPHA_FAINT: u8 = 2;

/// Syntax in the terminal's own sixteen colours, so it follows whatever theme it has. A colour
/// with alpha 0 names a palette index in `r`, as bat's `ansi` theme does.
fn theme() -> Theme {
    let indexed = |index: u8| Color {
        r: index,
        g: 0,
        b: 0,
        a: ALPHA_INDEXED,
    };
    let marker = |alpha: u8| Color {
        r: 0,
        g: 0,
        b: 0,
        a: alpha,
    };
    let rules = [
        (
            "keyword, storage, entity.name.tag, markup.heading, keyword.operator.word",
            indexed(5),
        ),
        ("keyword.operator, punctuation", marker(ALPHA_DEFAULT)),
        (
            "string, string.regexp, constant.other.symbol, markup.underline.link",
            indexed(2),
        ),
        (
            "constant.numeric, constant.language, constant.character, markup.list",
            indexed(3),
        ),
        (
            "entity.name.function, support.function, variable.function",
            indexed(4),
        ),
        (
            "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
             entity.name.trait, entity.other.inherited-class, support.type, support.class, \
             storage.type.primitive, storage.type.numeric",
            indexed(3),
        ),
        (
            "entity.other.attribute-name, variable.other.member, variable.other.property, \
             support.type.property-name, meta.mapping.key string, meta.object-literal.key, \
             meta.annotation, meta.attribute, meta.preprocessor",
            indexed(6),
        ),
    ];
    let mut scopes: Vec<ThemeItem> = rules
        .into_iter()
        .map(|(selectors, foreground)| ThemeItem {
            scope: selectors
                .parse::<ScopeSelectors>()
                .expect("valid selectors"),
            style: StyleModifier {
                foreground: Some(foreground),
                ..Default::default()
            },
        })
        .collect();
    scopes.push(ThemeItem {
        scope: "comment, punctuation.definition.comment"
            .parse()
            .expect("valid selectors"),
        style: StyleModifier {
            foreground: Some(marker(ALPHA_FAINT)),
            font_style: Some(FontStyle::ITALIC),
            ..Default::default()
        },
    });
    Theme {
        settings: ThemeSettings {
            foreground: Some(marker(ALPHA_DEFAULT)),
            ..Default::default()
        },
        scopes,
        ..Default::default()
    }
}

fn tone_of(color: Color) -> Tone {
    match color.a {
        ALPHA_INDEXED => Tone::Indexed(color.r),
        ALPHA_FAINT => Tone::Faint,
        _ => Tone::Default,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
    pub italic: bool,
}

impl Span {
    pub fn plain(text: &str) -> Self {
        Span {
            text: text.to_string(),
            tone: Tone::Default,
            italic: false,
        }
    }
}

/// Lines this long are minified or generated; highlighting them costs more than it shows.
const LONGEST_HIGHLIGHTED: usize = 2000;

pub struct Highlighter {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl Highlighter {
    pub fn new() -> Self {
        Highlighter {
            syntaxes: two_face::syntax::extra_newlines(),
            theme: theme(),
        }
    }

    fn syntax(&self, path: &str, first_line: &str) -> &SyntaxReference {
        let path = Path::new(path);
        let name = path.file_name().and_then(|name| name.to_str());
        let extension = path.extension().and_then(|extension| extension.to_str());
        [name, extension]
            .into_iter()
            .flatten()
            .find_map(|token| self.syntaxes.find_syntax_by_extension(token))
            .or_else(|| self.syntaxes.find_syntax_by_first_line(first_line))
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text())
    }

    /// Consecutive lines of one side of a hunk, so a construct spanning lines keeps its colour.
    pub fn lines(&self, path: &str, lines: &[&str]) -> Vec<Vec<Span>> {
        let syntax = self.syntax(path, lines.first().copied().unwrap_or_default());
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        lines
            .iter()
            .map(|line| {
                if line.len() > LONGEST_HIGHLIGHTED {
                    return vec![Span::plain(line)];
                }
                let with_newline = format!("{line}\n");
                match highlighter.highlight_line(&with_newline, &self.syntaxes) {
                    Ok(regions) => regions
                        .into_iter()
                        .map(|(style, text)| Span {
                            text: text.trim_end_matches('\n').to_string(),
                            tone: tone_of(style.foreground),
                            italic: style.font_style.contains(FontStyle::ITALIC),
                        })
                        .filter(|span| !span.text.is_empty())
                        .collect(),
                    Err(_) => vec![Span::plain(line)],
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_code_with_the_terminal_palette() {
        let highlighter = Highlighter::new();
        let lines = highlighter.lines("src/a.rs", &["fn main() { let n = 42; } // done"]);
        let span_of = |text: &str| {
            lines[0]
                .iter()
                .find(|span| span.text.contains(text))
                .cloned()
                .unwrap()
        };
        assert_eq!(span_of("fn").tone, Tone::Indexed(5));
        assert_eq!(span_of("42").tone, Tone::Indexed(3));
        assert_eq!(span_of("done").tone, Tone::Faint);
        assert!(span_of("done").italic);
        let joined: String = lines[0].iter().map(|span| span.text.as_str()).collect();
        assert_eq!(joined, "fn main() { let n = 42; } // done");

        let typescript = highlighter.lines("src/a.tsx", &["const name: string = 'seam';"]);
        assert!(typescript[0]
            .iter()
            .any(|span| span.text.contains("seam") && span.tone == Tone::Indexed(2)));
    }

    #[test]
    fn tints_only_over_a_known_background() {
        let dark = Palette::new(Some(Rgb(0x12, 0x13, 0x17)), true);
        assert_eq!(dark.added, Tone::Exact(hex(0x6cb6ff)));
        assert!(dark.added_background.is_some());

        let light = Palette::new(Some(Rgb(0xff, 0xff, 0xff)), true);
        assert_eq!(light.added, Tone::Exact(hex(0x0b5cad)));

        assert_eq!(Palette::new(None, true).added_background, None);
        assert_eq!(Palette::new(None, false).deleted, Tone::Indexed(209));
    }
}
