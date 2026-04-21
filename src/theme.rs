//! Color tokens and typography knobs for rendered diagrams.
//!
//! Default palette follows the editorial skin from
//! `robius/diagram-design/references/style-guide.md`:
//! warm stone paper, deep charcoal ink, rust accent. Consumers can swap by
//! building a custom [`Theme`] or picking [`Theme::light`] / [`Theme::dark`].

/// RGBA color in 0..=255 channels. Pre-multiplied-alpha is the renderer's
/// concern; this crate keeps them straight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Construct an opaque RGB color.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Construct a color with an explicit alpha channel.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a `#rrggbb` hex string at compile-time-ish (panics in const
    /// context when malformed).
    ///
    /// # Panics
    /// If the string does not match `#rrggbb`.
    #[must_use]
    pub const fn hex(s: &str) -> Self {
        let bytes = s.as_bytes();
        assert!(bytes.len() == 7 && bytes[0] == b'#', "expected #rrggbb");
        let r = hex_byte(bytes[1], bytes[2]);
        let g = hex_byte(bytes[3], bytes[4]);
        let b = hex_byte(bytes[5], bytes[6]);
        Self { r, g, b, a: 255 }
    }

    /// Return a copy with the alpha channel replaced.
    #[must_use]
    pub const fn with_alpha(mut self, a: u8) -> Self {
        self.a = a;
        self
    }
}

const fn hex_byte(hi: u8, lo: u8) -> u8 {
    hex_nibble(hi) * 16 + hex_nibble(lo)
}

const fn hex_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("non-hex digit in color literal"),
    }
}

/// Semantic color tokens. Typography is not color-linked; knobs live in
/// [`Typography`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Page / canvas background; default node fill.
    pub paper: Color,
    /// Primary text, primary stroke.
    pub ink: Color,
    /// Focal / accent (one per diagram).
    pub accent: Color,
    /// Secondary text, default arrow stroke.
    pub muted: Color,
    /// Hairline border color for dividers and rules.
    pub rule: Color,
}

/// Typography knobs. Sizes are in logical pixels (lpx).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    /// Primary node label size (Geist sans equivalent, ~12 lpx).
    pub label_size: f32,
    /// Eyebrow / sublabel size (Geist Mono equivalent, ~9 lpx).
    pub sublabel_size: f32,
    /// Arrow / axis annotation size.
    pub annotation_size: f32,
}

/// Visual theme: palette + typography + stroke widths.
///
/// Build your own by cloning and overriding fields:
///
/// ```
/// # use makepad_diagram_kit::Theme;
/// let mut mine = Theme::light();
/// mine.stroke_default = 1.5;
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub typography: Typography,
    /// Default stroke width for most borders (1.0 lpx).
    pub stroke_default: f32,
    /// Corner radius for rectangular nodes (6 lpx).
    pub corner_radius: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

impl Theme {
    /// Editorial light skin — the default.
    ///
    /// Palette: paper `#faf7f2`, ink `#1c1917`, accent `#b5523a`,
    /// muted `#78716c`, rule `#e7e5e4`.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            palette: Palette {
                paper: Color::hex("#faf7f2"),
                ink: Color::hex("#1c1917"),
                accent: Color::hex("#b5523a"),
                muted: Color::hex("#78716c"),
                rule: Color::hex("#e7e5e4"),
            },
            typography: Typography {
                label_size: 12.0,
                sublabel_size: 9.0,
                annotation_size: 8.0,
            },
            stroke_default: 1.0,
            corner_radius: 6.0,
        }
    }

    /// Editorial dark skin.
    ///
    /// Palette: paper `#1c1917`, ink `#faf7f2`, accent `#d97757`,
    /// muted `#a8a29e`, rule `#44403c`.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            palette: Palette {
                paper: Color::hex("#1c1917"),
                ink: Color::hex("#faf7f2"),
                accent: Color::hex("#d97757"),
                muted: Color::hex("#a8a29e"),
                rule: Color::hex("#44403c"),
            },
            typography: Typography {
                label_size: 12.0,
                sublabel_size: 9.0,
                annotation_size: 8.0,
            },
            stroke_default: 1.0,
            corner_radius: 6.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_default_paper() {
        let c = Color::hex("#faf7f2");
        assert_eq!(c, Color::rgb(0xfa, 0xf7, 0xf2));
    }

    #[test]
    fn light_and_dark_differ() {
        assert_ne!(Theme::light().palette.paper, Theme::dark().palette.paper);
    }
}
