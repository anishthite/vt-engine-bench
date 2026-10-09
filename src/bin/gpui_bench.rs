//! Visible-pixel GPUI comparison. Run through scripts/run_gpui_bench.sh, not directly.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use gpui::{
    div, prelude::*, px, rgb, App, AppContext, Application, Context, FocusHandle, IntoElement,
    Keystroke, Render, Window, WindowOptions,
};
use wezterm_term::{Terminal as WezTerm, TerminalConfiguration, TerminalSize};

const COLS: usize = 120;
const ROWS: usize = 40;
const OUTPUT_FRAMES: usize = 80;
const INPUT_FRAMES: usize = 40;
const TMUX: &[u8] = include_bytes!("../../fixtures/tmux-redraw.bin");
static RENDERS: AtomicUsize = AtomicUsize::new(0);
static SYNTHETIC_KEY: AtomicBool = AtomicBool::new(false);

fn millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.0
}

#[derive(Debug)]
struct BenchConfig;
impl TerminalConfiguration for BenchConfig {
    fn scrollback_size(&self) -> usize {
        1000
    }
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        Default::default()
    }
}

// Variant size is fixed at initialization; avoid boxing just to shrink this enum.
#[allow(clippy::large_enum_variant)]
enum Engine {
    Alacritty(Term<VoidListener>, Processor),
    Ghostty(ghostty_vt::Terminal),
    Vt100(vt100::Parser),
    Wezterm(WezTerm),
}
impl Engine {
    fn new(name: &str) -> Self {
        match name {
            "alacritty" => Self::Alacritty(
                Term::new(
                    Config {
                        scrolling_history: 1000,
                        ..Config::default()
                    },
                    &TermSize::new(COLS, ROWS),
                    VoidListener,
                ),
                Processor::new(),
            ),
            "ghostty" => Self::Ghostty(ghostty_vt::Terminal::new(COLS as u16, ROWS as u16)),
            "vt100" => Self::Vt100(vt100::Parser::new(ROWS as u16, COLS as u16, 1000)),
            "wezterm" => Self::Wezterm(WezTerm::new(
                TerminalSize {
                    rows: ROWS,
                    cols: COLS,
                    pixel_width: COLS * 8,
                    pixel_height: ROWS * 16,
                    dpi: 96,
                },
                Arc::new(BenchConfig),
                "bench",
                "0",
                Box::new(std::io::sink()),
            )),
            _ => panic!("engine must be alacritty, ghostty, vt100, or wezterm"),
        }
    }
    fn feed(&mut self, bytes: &[u8]) {
        match self {
            Self::Alacritty(term, parser) => parser.advance(term, bytes),
            Self::Ghostty(term) => term.feed(bytes),
            Self::Vt100(term) => term.process(bytes),
            Self::Wezterm(term) => term.advance_bytes(bytes),
        }
    }
    fn rows(&self) -> Vec<String> {
        match self {
            Self::Alacritty(term, _) => (0..ROWS)
                .map(|row| {
                    (&term.grid()[Line(row as i32)])
                        .into_iter()
                        .map(|cell| cell.c)
                        .collect::<String>()
                })
                .collect(),
            Self::Ghostty(term) => term.snapshot().0,
            Self::Vt100(term) => term.screen().rows(0, COLS as u16).collect(),
            Self::Wezterm(term) => term
                .screen()
                .lines_in_phys_range(0..ROWS)
                .iter()
                .map(|row| row.as_str().to_string())
                .collect(),
        }
    }
}

struct View {
    engine: Engine,
    rows: Vec<String>,
    focus: FocusHandle,
    // Cycle four colors to identify skipped captured frames.
    marker: u8,
    input_index: usize,
    events: Arc<Mutex<Vec<(String, usize, f64)>>>,
}
impl View {
    fn feed(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.engine.feed(bytes);
        self.rows = self.engine.rows();
        self.marker += 1;
        cx.notify();
    }
}
impl Render for View {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        RENDERS.fetch_add(1, Ordering::Relaxed);
        let color = [0x3b82f6, 0xef4444, 0x22c55e, 0xeab308][usize::from(self.marker % 4)];
        div()
            .size_full()
            .flex()
            .bg(rgb(0x0d1117))
            .text_color(rgb(0xc9d1d9))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if !SYNTHETIC_KEY.load(Ordering::Relaxed) {
                    return;
                }
                let Some(key) = event.keystroke.key_char.as_ref() else {
                    return;
                };
                let index = this.input_index;
                this.input_index += 1;
                let sent = millis();
                let bytes = format!("\x1b[1;1H\x1b[2KINPUT {index:03} {key}");
                this.feed(bytes.as_bytes(), cx);
                this.events
                    .lock()
                    .unwrap()
                    .push(("input".to_owned(), index, sent));
            }))
            .child(div().flex_shrink_0().w(px(96.0)).h_full().bg(rgb(color)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(13.0))
                    .font_family("Menlo")
                    .children(
                        self.rows
                            .iter()
                            .map(|row| div().h(px(16.0)).child(row.clone()))
                            .collect::<Vec<_>>(),
                    ),
            )
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let engine = args
        .next()
        .expect("usage: gpui_bench ENGINE events.csv ready-file");
    let events_path = args.next().expect("missing events.csv path");
    let ready = args.next().expect("missing capture-ready file path");
    let events = Arc::new(Mutex::new(Vec::new()));
    Application::new().run(move |cx: &mut App| {
        let view_events = events.clone();
        let view = cx
            .open_window(WindowOptions::default(), |window, cx| {
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                cx.new(|_| {
                    let engine = Engine::new(&engine);
                    let rows = engine.rows();
                    View {
                        engine,
                        rows,
                        focus,
                        marker: 0,
                        input_index: 0,
                        events: view_events.clone(),
                    }
                })
            })
            .expect("GPUI window");
        cx.activate(true);
        let log = events.clone();
        cx.spawn(async move |cx| {
            // No timing until the collector is attached to this exact window.
            for _ in 0..150 {
                if std::path::Path::new(&ready).exists() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
            if !std::path::Path::new(&ready).exists() {
                eprintln!("ScreenCaptureKit collector did not attach; no benchmark produced");
                cx.update(|cx| cx.quit()).ok();
                return;
            }
            for index in 0..OUTPUT_FRAMES {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let start = index * TMUX.len() / OUTPUT_FRAMES;
                let end = (index + 1) * TMUX.len() / OUTPUT_FRAMES;
                cx.update(|cx| {
                    view.update(cx, |this, window, cx| {
                        let sent = millis();
                        this.feed(&TMUX[start..end], cx);
                        log.lock().unwrap().push(("tmux".to_owned(), index, sent));
                        window.refresh();
                    })
                    .expect("GPUI update");
                })
                .expect("app update");
            }
            cx.update(|cx| {
                view.update(cx, |this, _, _| {
                    let mut rows: Vec<_> = this
                        .rows
                        .iter()
                        .map(|row| row.trim_end().to_owned())
                        .collect();
                    while rows.last().is_some_and(String::is_empty) {
                        rows.pop();
                    }
                    let expected: Vec<String> = std::iter::once("frame 0150".to_owned())
                        .chain((1..=38).map(|i| i.to_string()))
                        .collect();
                    assert_eq!(rows, expected, "GPUI engine viewport diverged");
                })
                .expect("viewport check");
            })
            .expect("app update");
            // Route synthetic keys through GPUI's key-dispatch and real view handler.
            for _ in 0..INPUT_FRAMES {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                cx.update(|cx| {
                    cx.update_window(view.into(), |_, window, cx| {
                        SYNTHETIC_KEY.store(true, Ordering::Relaxed);
                        window.dispatch_keystroke(Keystroke::parse("a").unwrap(), cx);
                        SYNTHETIC_KEY.store(false, Ordering::Relaxed);
                        window.refresh();
                    })
                    .expect("GPUI key dispatch");
                })
                .expect("app update");
            }
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            cx.update(|cx| {
                let events = log.lock().unwrap();
                let mut csv = String::from("phase,index,sent_epoch_ms\n");
                for (phase, index, sent) in events.iter() {
                    csv.push_str(&format!("{phase},{index},{sent:.4}\n"));
                }
                std::fs::write(&events_path, csv).expect("write events");
                eprintln!(
                    "{}: {} dispatched updates, {} GPUI renders",
                    engine,
                    events.len(),
                    RENDERS.load(Ordering::Relaxed)
                );
                cx.quit();
            })
            .expect("app update");
        })
        .detach();
    });
}
