#[cfg(test)]
mod compat;

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use wezterm_term::{Terminal as WezTerm, TerminalConfiguration, TerminalSize};
#[derive(Debug)]
struct BenchConfig;
struct Noop;
impl vte::Perform for Noop {}
impl TerminalConfiguration for BenchConfig {
    fn scrollback_size(&self) -> usize {
        1000
    }
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        Default::default()
    }
}

const COLS: usize = 120;
const ROWS: usize = 40;
const ROUNDS: usize = 7;

fn workloads() -> Vec<(&'static str, Vec<u8>)> {
    let mut plain = Vec::new();
    let mut styled = Vec::new();
    let mut redraw = Vec::new();
    for i in 0..15_000 {
        plain.extend_from_slice(
            format!("{i:05} The quick brown fox jumps over the lazy dog.\r\n").as_bytes(),
        );
        styled.extend_from_slice(
            format!("\x1b[38;5;{}m{i:05} colored output\x1b[0m\r\n", i % 256).as_bytes(),
        );
    }
    for i in 0..10_000 {
        redraw.extend_from_slice(
            format!(
                "\x1b[{};1H\x1b[2K\x1b[3{}mframe {i:05} αβγ",
                i % ROWS + 1,
                i % 8
            )
            .as_bytes(),
        );
    }
    let tmux = include_bytes!("../fixtures/tmux-redraw.bin").repeat(10);
    vec![
        ("plain-scroll", plain),
        ("ansi-scroll", styled),
        ("tui-redraw", redraw),
        ("tmux-client", tmux),
    ]
}

fn time(name: &str, workload: &str, bytes: &[u8], mut run: impl FnMut(&[u8])) {
    let mut samples = Vec::new();
    for _ in 0..ROUNDS {
        let start = Instant::now();
        run(black_box(bytes));
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{workload:14} {name:20} {:>9.2} ms   {:>8.1} MiB/s",
        samples[ROUNDS / 2],
        bytes.len() as f64 / samples[ROUNDS / 2] / 1048.576
    );
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    #[test]
    fn workloads_cover_scroll_color_and_cursor_redraw() {
        let inputs = super::workloads();
        assert_eq!(inputs.len(), 4);
        assert!(inputs[0].1.windows(2).any(|bytes| bytes == b"\r\n"));
        assert!(inputs[1].1.windows(5).any(|bytes| bytes == b"\x1b[38;"));
        assert!(inputs[2].1.windows(4).any(|bytes| bytes == b"\x1b[2K"));
        assert!(inputs[3].1.windows(5).any(|bytes| bytes == b"frame"));
    }

    #[test]
    fn all_engines_render_tmux_output() {
        use super::*;
        let bytes = workloads().pop().unwrap().1;
        let expected: Vec<String> = std::iter::once("frame 0150".to_string())
            .chain((1..=38).map(|n| n.to_string()))
            .collect();
        let normalize = |rows: Vec<String>| -> Vec<String> {
            let mut rows: Vec<_> = rows
                .into_iter()
                .map(|row| row.trim_end().to_string())
                .collect();
            while rows.last().is_some_and(String::is_empty) {
                rows.pop();
            }
            rows
        };

        let mut alacritty = Term::new(Config::default(), &TermSize::new(COLS, ROWS), VoidListener);
        let mut parser: Processor = Processor::new();
        for chunk in bytes.chunks(8192) {
            parser.advance(&mut alacritty, chunk);
        }
        use alacritty_terminal::index::Line;
        let alacritty_rows = normalize(
            (0..ROWS)
                .map(|row| {
                    (&alacritty.grid()[Line(row as i32)])
                        .into_iter()
                        .map(|cell| cell.c)
                        .collect()
                })
                .collect(),
        );
        assert_eq!(alacritty_rows, expected, "alacritty viewport");

        let mut ghostty = ghostty_vt::Terminal::new(COLS as u16, ROWS as u16).unwrap();
        for chunk in bytes.chunks(8192) {
            ghostty.feed(chunk).unwrap();
        }
        let ghostty_rows = normalize(
            ghostty
                .dump_viewport()
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(ghostty_rows, expected, "ghostty viewport");

        let mut vt100 = vt100::Parser::new(ROWS as u16, COLS as u16, 1000);
        for chunk in bytes.chunks(8192) {
            vt100.process(chunk);
        }
        let vt100_rows = normalize(vt100.screen().rows(0, COLS as u16).collect());
        assert_eq!(vt100_rows, expected, "vt100 viewport");

        let size = TerminalSize {
            rows: ROWS,
            cols: COLS,
            pixel_width: COLS * 8,
            pixel_height: ROWS * 16,
            dpi: 96,
        };
        let mut wezterm = WezTerm::new(
            size,
            Arc::new(BenchConfig),
            "bench",
            "0",
            Box::new(std::io::sink()),
        );
        for chunk in bytes.chunks(8192) {
            wezterm.advance_bytes(chunk);
        }
        let wezterm_rows = normalize(
            wezterm
                .screen()
                .lines_in_phys_range(0..ROWS)
                .iter()
                .map(|row| row.as_str().to_string())
                .collect(),
        );
        assert_eq!(wezterm_rows, expected, "wezterm viewport");
    }
}

fn main() {
    println!(
        "Headless parse + screen-state; {} runs, median; {}x{}; no GUI or PTY",
        ROUNDS, COLS, ROWS
    );
    for (label, bytes) in workloads() {
        println!("{} bytes: {}", label, bytes.len());
        time("alacritty_terminal", label, &bytes, |bytes| {
            let mut term = Term::new(Config::default(), &TermSize::new(COLS, ROWS), VoidListener);
            let mut parser: Processor = Processor::new();
            for chunk in bytes.chunks(8192) {
                parser.advance(&mut term, chunk);
            }
            black_box(term);
        });
        time("ghostty-vt-1.2.3", label, &bytes, |bytes| {
            let mut term = ghostty_vt::Terminal::new(COLS as u16, ROWS as u16).unwrap();
            for chunk in bytes.chunks(8192) {
                term.feed(chunk).unwrap();
            }
            black_box(term);
        });
        time("vt100", label, &bytes, |bytes| {
            let mut parser = vt100::Parser::new(ROWS as u16, COLS as u16, 1000);
            for chunk in bytes.chunks(8192) {
                parser.process(chunk);
            }
            black_box(parser);
        });
        time("wezterm-term", label, &bytes, |bytes| {
            let size = TerminalSize {
                rows: ROWS,
                cols: COLS,
                pixel_width: COLS * 8,
                pixel_height: ROWS * 16,
                dpi: 96,
            };
            let mut term = WezTerm::new(
                size,
                Arc::new(BenchConfig),
                "bench",
                "0",
                Box::new(std::io::sink()),
            );
            for chunk in bytes.chunks(8192) {
                term.advance_bytes(chunk);
            }
            black_box(term);
        });
        // Parser-only baseline: no grid or screen state; never rank against engines.
        time("vte (parser only)", label, &bytes, |bytes| {
            let mut parser = vte::Parser::new();
            let mut sink = Noop;
            for chunk in bytes.chunks(8192) {
                parser.advance(&mut sink, chunk);
            }
            black_box(parser);
        });
    }
}
