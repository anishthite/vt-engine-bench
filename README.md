# VT engine benchmark (macOS)

Headless **parsing + screen-state update**, not terminal-rendering speed. Sources are pinned as git submodules (see `git submodule status` and `Cargo.lock`). Ghostty is pinned to current `main` at [`b699ea7`](https://github.com/ghostty-org/ghostty/commit/b699ea79f4b881421b4b3055abc16a0957d76beb) (1.3.2-dev). Its benchmark-only C shim is compiled against that checkout's headers and links the matching `libghostty-vt` archive; `libghostty-rs` supplies the Zig build integration, **not** its older Rust API. The other engines are pinned source checkouts. Parser-only `vte` is excluded because it has no screen state.

```sh
git submodule update --init --recursive
./bootstrap-zig.sh # Zig 0.16.0
PATH="$PWD/.context/zig:$PATH" cargo test
PATH="$PWD/.context/zig:$PATH" cargo run --release
```

Three workloads are generated in `src/main.rs`: 15k plain scroll lines, 15k 256-color lines, and 10k cursor-positioned/cleared TUI updates. A fourth is a **real tmux client PTY recording** (`fixtures/tmux-redraw.bin`, 25,641 bytes, SHA-256 `d30383bb7658ae386f6f8e70e7d26bc8f1484b95d1199cbaf81715ad7233b7da`) captured by `python3 scripts/capture_tmux.py`, replayed ten times for timing. Re-capturing changes the fixture; the committed bytes define this run. The capture uses `/bin/sh`, a temporary socket and config, and disables tmux's status line to avoid machine-identifying data.

120×40 cells, 1k scrollback in each engine where configurable, chunks of 8192 bytes. Seven runs per workload/engine, median wall time includes creating a fresh model and dropping it; throughput uses input bytes. No GUI, live PTY during timing, glyph rendering, font shaping, or latency. Run when machine is otherwise idle. Higher throughput here **does not mean smoother or faster terminal UI**. Different protocol feature sets and implementation paths mean this is exploratory, not a compliance-normalized comparison. The Ghostty C shim intentionally exposes only APIs the benchmark tests; do not treat it as a general-purpose Rust binding. `cargo test` checks **all four engines reproduce the exact expected visible text** (the `frame 0150` header and 38 numbered lines) after the same 8192-byte chunking and ten replays used in timing. Additional small-screen fixtures in `src/compat.rs` check the behaviors below with 7-byte chunking (splitting ANSI escapes and UTF-8). These checks do not establish broad VT compliance. The previous recording included a shell prompt after the last frame; it caused a scroll and divergent end state. The capture script now keeps the command running while recording the final frame, removing that ambiguous post-command transition. Replaying ten times also repeats initialization sequences; it is not a continuous tmux session.

## Core compatibility checks

| Check | Alacritty | Ghostty main | vt100 | WezTerm |
|---|:---:|:---:|:---:|:---:|
| Repeated tmux client: exact visible text | ✓ | ✓ | ✓ | ✓ |
| Cursor addressing and line clear | ✓ | ✓ | ✓ | ✓ |
| Truecolor foreground/background and bold | ✓ | ✓ | ✓ | ✓ |
| Indexed color and alternate-screen restore | ✓ | ✓ | ✓ | ✓ |
| Wide Unicode and combining marks | ✓ | ✓ | ✓ | ✓ |
| Preserve text on resize | ✓ | ✓ | ✓ | ✓ |
| Retain and inspect scrollback | ✓ | ✓ | ✓ | ✓ |

Only the **specific fixtures** pass; this is not a complete VT conformance suite. In particular, mouse input, selection, OSC clipboard, hyperlinks, grapheme clustering, error handling, exact cursor shape, rendering, and latency remain untested. The Ghostty column tests the pinned current-main C API through a narrow benchmark shim, not the full macOS app.

## Recorded run (Apple M1 Pro, macOS 15.4.1, release build, 2026-10-06)

| Workload | alacritty_terminal | Ghostty main | vt100 | wezterm-term |
|---|---:|---:|---:|---:|
| plain scroll, ms | 5.98 | **1.34** | 7.93 | 26.24 |
| ANSI scroll, ms | **4.67** | 6.18 | 6.06 | 14.64 |
| TUI redraw, ms | 3.56 | **2.66** | 3.51 | 25.17 |
| tmux client (repeated recording), ms | **2.14** | 6.02 | 8.54 | 138.26 |

[Raw per-run medians](results/2026-10-06-m1-pro-current-ghostty.csv) are included. Values are the **median of three independent process runs**, each reporting the median of seven iterations. This is not a controlled system-wide benchmark. [Previous runs](results/2026-10-06-m1-pro.csv) used Ghostty 1.2.3 and Alacritty's 10k-line default scrollback; **do not compare them directly** to these 1k-line results.

**Do not select a GPUI terminal core on these numbers alone.** All four pass the bounded core fixtures; none is proven the fastest rendered terminal. Ghostty leads plain output and synthetic TUI redraws; Alacritty leads this recorded tmux stream and ANSI output.

## GPUI display and input (not measured)

| Metric | Comparable result |
|---|---|
| GPUI rendering / presented frames | Not measured |
| Frame pacing (presented-frame intervals) | Not measured |
| Input-to-screen latency | Not measured |

A GPUI `on_next_frame` callback runs **before** draw/present; it cannot report any of these metrics. A local GUI prototype also failed to produce distinct active frames from this agent's shell session, so its queued callback timings were discarded. These require a working foreground window and a presentation or pixel-observation instrument; keyboard-to-pixel latency additionally needs input event timestamps. The headless figures above cannot substitute for them.
