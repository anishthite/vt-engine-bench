//! Bounded interoperability checks for a one-pane tmux-oriented terminal.
use super::*;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::vte::ansi::{Color as AlColor, Rgb as AlRgb};
use ghostty_vt::Terminal as GhosttyTerminal;
use wezterm_cell::color::ColorAttribute;

struct Engines {
    alacritty: Term<VoidListener>,
    parser: Processor,
    ghostty: GhosttyTerminal,
    vt100: vt100::Parser,
    wezterm: WezTerm,
}

impl Engines {
    fn new(cols: usize, rows: usize) -> Self {
        let size = TerminalSize {
            rows,
            cols,
            pixel_width: cols * 8,
            pixel_height: rows * 16,
            dpi: 96,
        };
        let ghostty = GhosttyTerminal::new(cols as u16, rows as u16);
        Self {
            alacritty: Term::new(alacritty_config(), &TermSize::new(cols, rows), VoidListener),
            parser: Processor::new(),
            ghostty,
            vt100: vt100::Parser::new(rows as u16, cols as u16, 1000),
            wezterm: WezTerm::new(
                size,
                Arc::new(BenchConfig),
                "bench",
                "0",
                Box::new(std::io::sink()),
            ),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        // Split inside escape sequences and UTF-8 characters as a real PTY may do.
        for chunk in bytes.chunks(7) {
            self.parser.advance(&mut self.alacritty, chunk);
            self.ghostty.feed(chunk);
            self.vt100.process(chunk);
            self.wezterm.advance_bytes(chunk);
        }
    }

    fn rows(&self, cols: usize, rows: usize) -> [Vec<String>; 4] {
        let alacritty: Vec<String> = (0..rows)
            .map(|y| {
                (&self.alacritty.grid()[Line(y as i32)])
                    .into_iter()
                    .filter(|cell| {
                        !cell
                            .flags
                            .contains(alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER)
                    })
                    .map(|cell| {
                        let mut text = cell.c.to_string();
                        if let Some(remaining) = cell.zerowidth() {
                            text.extend(remaining.iter());
                        }
                        text
                    })
                    .collect()
            })
            .collect();
        let ghostty = self.ghostty.snapshot().0;
        let vt100 = self.vt100.screen().rows(0, cols as u16).collect();
        let wezterm = self
            .wezterm
            .screen()
            .lines_in_phys_range(0..rows)
            .iter()
            .map(|line| line.as_str().to_string())
            .collect();
        [alacritty, ghostty, vt100, wezterm].map(|rows: Vec<String>| {
            rows.into_iter()
                .map(|row| row.trim_end().to_owned())
                .collect()
        })
    }

    fn resize(&mut self, cols: usize, rows: usize) {
        self.alacritty.resize(TermSize::new(cols, rows));
        self.ghostty.resize(cols as u16, rows as u16);
        self.vt100.screen_mut().set_size(rows as u16, cols as u16);
        self.wezterm.resize(TerminalSize {
            rows,
            cols,
            pixel_width: cols * 8,
            pixel_height: rows * 16,
            dpi: 96,
        });
    }
}

#[test]
fn cursor_and_clear() {
    let mut engines = Engines::new(12, 4);
    engines.feed(b"\x1b[2;4HZ\x1b[3;2H\x1b[0K");
    for (i, rows) in engines.rows(12, 4).into_iter().enumerate() {
        assert_eq!(rows[1], "   Z", "engine {i} text");
    }
    let a = engines.alacritty.grid().cursor.point;
    assert_eq!((a.column.0, a.line.0), (1, 2));
    assert_eq!(engines.ghostty.cursor(), (1, 2));
    assert_eq!(engines.vt100.screen().cursor_position(), (2, 1));
    let w = engines.wezterm.cursor_pos();
    assert_eq!((w.x, w.y), (1, 2));
}

#[test]
fn truecolor_and_bold() {
    let mut engines = Engines::new(12, 4);
    engines.feed(b"\x1b[1;38;2;17;34;51;48;2;68;85;102mX\x1b[0mY");
    let a = &engines.alacritty.grid()[Line(0)][Column(0)];
    assert_eq!(
        a.fg,
        AlColor::Spec(AlRgb {
            r: 17,
            g: 34,
            b: 51
        })
    );
    assert_eq!(
        a.bg,
        AlColor::Spec(AlRgb {
            r: 68,
            g: 85,
            b: 102
        })
    );
    assert!(a
        .flags
        .contains(alacritty_terminal::term::cell::Flags::BOLD));
    let (_, fg, bg) = engines.ghostty.snapshot();
    assert_eq!(fg.valid, 1);
    assert_eq!(bg.valid, 1);
    let bold = fg.bold != 0;
    assert_eq!((fg.r, fg.g, fg.b), (17, 34, 51));
    assert_eq!((bg.r, bg.g, bg.b), (68, 85, 102));
    assert!(bold, "ghostty bold");
    let v = engines.vt100.screen().cell(0, 0).unwrap();
    assert_eq!(v.fgcolor(), vt100::Color::Rgb(17, 34, 51));
    assert_eq!(v.bgcolor(), vt100::Color::Rgb(68, 85, 102));
    assert!(v.bold());
    let w = engines.wezterm.screen().lines_in_phys_range(0..1);
    let cell = w[0].get_cell(0).unwrap();
    assert_eq!(cell.attrs().intensity(), wezterm_cell::Intensity::Bold);
    match cell.attrs().foreground() {
        ColorAttribute::TrueColorWithDefaultFallback(rgb) => {
            assert_eq!(rgb.as_rgba_u8(), (17, 34, 51, 255))
        }
        other => panic!("wezterm fg: {other:?}"),
    }
    match cell.attrs().background() {
        ColorAttribute::TrueColorWithDefaultFallback(rgb) => {
            assert_eq!(rgb.as_rgba_u8(), (68, 85, 102, 255))
        }
        other => panic!("wezterm bg: {other:?}"),
    }
    for (i, rows) in engines.rows(12, 4).into_iter().enumerate() {
        assert_eq!(rows[0], "XY", "engine {i} colors text");
    }
}

#[test]
fn indexed_color_and_alternate_screen() {
    let mut engines = Engines::new(12, 4);
    engines.feed(b"NORMAL\x1b[?1049h\x1b[H\x1b[38;5;196mR");
    let a = &engines.alacritty.grid()[Line(0)][Column(0)];
    assert_eq!(a.fg, AlColor::Indexed(196));
    let fg = engines.ghostty.snapshot().1;
    assert_eq!(fg.valid, 1);
    assert_eq!((fg.r, fg.g, fg.b), (255, 0, 0));
    assert_eq!(
        engines.vt100.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(196)
    );
    let w = engines.wezterm.screen().lines_in_phys_range(0..1);
    assert_eq!(
        w[0].get_cell(0).unwrap().attrs().foreground(),
        ColorAttribute::PaletteIndex(196)
    );
    for (i, rows) in engines.rows(12, 4).into_iter().enumerate() {
        assert_eq!(rows[0], "R", "engine {i} alternate screen");
    }
    engines.feed(b"\x1b[?1049l");
    for (i, rows) in engines.rows(12, 4).into_iter().enumerate() {
        assert_eq!(rows[0], "NORMAL", "engine {i} restored screen");
    }
}

#[test]
fn unicode_wide_and_combining() {
    let mut engines = Engines::new(12, 4);
    engines.feed("A界e\u{301}Z".as_bytes());
    for (i, rows) in engines.rows(12, 4).into_iter().enumerate() {
        assert!(
            rows[0].starts_with("A界e\u{301}Z"),
            "engine {i}: {:?}",
            rows[0]
        );
    }
}

#[test]
fn resize_preserves_simple_text() {
    let mut engines = Engines::new(12, 4);
    engines.feed(b"HELLO");
    engines.resize(8, 5);
    for (i, rows) in engines.rows(8, 5).into_iter().enumerate() {
        assert!(
            rows.iter().any(|r| r.contains("HELLO")),
            "engine {i}: {rows:?}"
        );
    }
}

#[test]
fn scrollback_retains_earliest_line() {
    let mut engines = Engines::new(12, 4);
    engines.feed(b"FIRST\r\nsecond\r\nthird\r\nfourth\r\nfifth\r\nsixth");
    let history = engines.alacritty.grid().history_size();
    assert!(history > 0);
    let earliest: String = (&engines.alacritty.grid()[Line(-(history as i32))])
        .into_iter()
        .map(|cell| cell.c)
        .collect();
    assert!(
        earliest.contains("FIRST"),
        "alacritty history: {earliest:?}"
    );
    assert!(engines.wezterm.screen().scrollback_rows() > 4);
    let oldest = engines.wezterm.screen().lines_in_phys_range(0..1);
    assert!(
        oldest[0].as_str().contains("FIRST"),
        "wezterm history: {:?}",
        oldest[0].as_str()
    );
    // vt100::scrollback reports the current view offset, not retained history.
    engines.ghostty.scroll_top();
    assert!(engines.ghostty.snapshot().0.join("\n").contains("FIRST"));
    engines.vt100.screen_mut().set_scrollback(2);
    assert!(engines.vt100.screen().contents().contains("FIRST"));
}
