//! Colour themes for the REPL. The palettes are the presets from
//! [ratatui-cheese](https://github.com/shashanktomar/ratatui-cheese) (MIT), whose
//! `charm` preset follows Charmbracelet's lipgloss examples; `number` is an extra
//! role for numeric literals.

use nu_ansi_term::{Color, Style};

#[derive(Clone, Debug)]
pub struct Palette {
    pub name: &'static str,
    pub foreground: Color,
    pub muted: Color,
    pub faint: Color,
    pub primary: Color,
    pub secondary: Color,
    pub highlight: Color,
    pub on_highlight: Color,
    pub error: Color,
    pub success: Color,
    pub number: Color,
}

pub const NAMES: &[&str] = &["charm", "dark", "light", "ocean", "sunset"];

impl Palette {
    pub fn named(name: &str) -> Option<Palette> {
        Some(match name {
            "charm" => Palette {
                name: "charm",
                foreground: Color::Rgb(0xFA, 0xFA, 0xFA),
                muted: Color::Fixed(250),
                faint: Color::Fixed(240),
                primary: Color::Rgb(0xEE, 0x6F, 0xF8),
                secondary: Color::Rgb(0x7D, 0x56, 0xF4),
                highlight: Color::Rgb(0x7D, 0x56, 0xF4),
                on_highlight: Color::Rgb(0xFA, 0xFA, 0xFA),
                error: Color::Rgb(0xFF, 0x55, 0x55),
                success: Color::Rgb(0x3D, 0xD6, 0x8C),
                number: Color::Rgb(0xFF, 0xD7, 0x87),
            },
            "dark" => Palette {
                name: "dark",
                foreground: Color::Fixed(252),
                muted: Color::Fixed(245),
                faint: Color::Fixed(239),
                primary: Color::Fixed(212),
                secondary: Color::Fixed(141),
                highlight: Color::Fixed(141),
                on_highlight: Color::Fixed(235),
                error: Color::Fixed(203),
                success: Color::Fixed(114),
                number: Color::Fixed(221),
            },
            "light" => Palette {
                name: "light",
                foreground: Color::Fixed(235),
                muted: Color::Fixed(243),
                faint: Color::Fixed(248),
                primary: Color::Fixed(162),
                secondary: Color::Fixed(97),
                highlight: Color::Fixed(97),
                on_highlight: Color::Fixed(255),
                error: Color::Fixed(160),
                success: Color::Fixed(28),
                number: Color::Fixed(130),
            },
            "ocean" => Palette {
                name: "ocean",
                foreground: Color::Rgb(0xE0, 0xE0, 0xE0),
                muted: Color::Fixed(246),
                faint: Color::Fixed(240),
                primary: Color::Rgb(0x00, 0xD7, 0x87),
                secondary: Color::Rgb(0x5F, 0xAF, 0xFF),
                highlight: Color::Rgb(0x5F, 0xAF, 0xFF),
                on_highlight: Color::Fixed(235),
                error: Color::Rgb(0xFF, 0x6B, 0x6B),
                success: Color::Rgb(0x00, 0xD7, 0x87),
                number: Color::Rgb(0xFF, 0xD7, 0x87),
            },
            "sunset" => Palette {
                name: "sunset",
                foreground: Color::Rgb(0xFA, 0xF0, 0xE6),
                muted: Color::Fixed(248),
                faint: Color::Fixed(242),
                primary: Color::Rgb(0xFF, 0x87, 0x5F),
                secondary: Color::Rgb(0xFF, 0xD7, 0x00),
                highlight: Color::Rgb(0xFF, 0x87, 0x5F),
                on_highlight: Color::Fixed(235),
                error: Color::Rgb(0xFF, 0x5F, 0x5F),
                success: Color::Rgb(0x87, 0xD7, 0x5F),
                number: Color::Rgb(0x87, 0xD7, 0xFF),
            },
            _ => return None,
        })
    }
}

/// A palette resolved into the styles the REPL paints with. `plain` (NO_COLOR, or
/// output that isn't a terminal) makes every style the default.
#[derive(Clone, Debug)]
pub struct Theme {
    pub palette: Palette,
    pub plain: bool,
}

impl Theme {
    pub fn new(palette: Palette, plain: bool) -> Theme {
        Theme { palette, plain }
    }

    fn style(&self, s: Style) -> Style {
        if self.plain {
            Style::new()
        } else {
            s
        }
    }

    /// Paint `text` with `s` (or leave it alone when plain).
    pub fn paint(&self, s: Style, text: impl AsRef<str>) -> String {
        if self.plain {
            text.as_ref().to_string()
        } else {
            s.paint(text.as_ref()).to_string()
        }
    }

    pub fn fg(&self) -> Style {
        self.style(Style::new().fg(self.palette.foreground))
    }
    pub fn muted(&self) -> Style {
        self.style(Style::new().fg(self.palette.muted))
    }
    pub fn faint(&self) -> Style {
        self.style(Style::new().fg(self.palette.faint))
    }
    pub fn primary(&self) -> Style {
        self.style(Style::new().fg(self.palette.primary))
    }
    pub fn secondary(&self) -> Style {
        self.style(Style::new().fg(self.palette.secondary))
    }
    pub fn error(&self) -> Style {
        self.style(Style::new().fg(self.palette.error))
    }
    pub fn success(&self) -> Style {
        self.style(Style::new().fg(self.palette.success))
    }
    pub fn number(&self) -> Style {
        self.style(Style::new().fg(self.palette.number))
    }
    /// A lipgloss-style badge: bold text on the highlight colour.
    pub fn badge(&self) -> Style {
        self.style(
            Style::new()
                .fg(self.palette.on_highlight)
                .on(self.palette.highlight)
                .bold(),
        )
    }
    /// The selected row of the completion menu.
    pub fn selected(&self) -> Style {
        self.style(
            Style::new()
                .fg(self.palette.on_highlight)
                .on(self.palette.highlight),
        )
    }

    /// Style for a semantic token type (`silt_lua::lsp::SEMANTIC_TOKEN_TYPES`).
    pub fn token(&self, kind: &str) -> Style {
        let p = &self.palette;
        self.style(match kind {
            "keyword" => Style::new().fg(p.primary).bold(),
            "string" => Style::new().fg(p.success),
            "number" => Style::new().fg(p.number),
            "comment" => Style::new().fg(p.faint).italic(),
            "operator" => Style::new().fg(p.muted),
            "function" | "method" => Style::new().fg(p.secondary).bold(),
            "parameter" => Style::new().fg(p.foreground).italic(),
            "type" => Style::new().fg(p.secondary).italic(),
            _ => Style::new().fg(p.foreground),
        })
    }
}
