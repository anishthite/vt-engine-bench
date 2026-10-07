# VT engine benchmark (macOS)

Headless **parsing + screen-state update**, not terminal-rendering speed. Sources are pinned as git submodules (see `git submodule status` and `Cargo.lock`). Ghostty uses the Apache-2.0 Zig/C ABI + Rust wrapper adapted from [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty), against Ghostty v1.2.3. The other engines are recent source checkouts. `vte` has **no screen model** and is included as an explicitly incomparable parser-only baseline.

```sh
git submodule update --init --recursive
./bootstrap-zig.sh # Zig 0.14.1, only if not installed
cargo run --release # or ZIG=/path/to/zig cargo run --release
```

Three workloads are generated in `src/main.rs`: 15k plain scroll lines, 15k 256-color lines, and 10k cursor-positioned/cleared TUI updates. A fourth is a **real tmux client PTY recording** (`fixtures/tmux-redraw.bin`, 25,641 bytes, SHA-256 `d30383bb7658ae386f6f8e70e7d26bc8f1484b95d1199cbaf81715ad7233b7da`) captured by `python3 scripts/capture_tmux.py`, replayed ten times for timing. Re-capturing changes the fixture; the committed bytes define this run. The capture uses `/bin/sh`, a temporary socket and config, and disables tmux's status line to avoid machine-identifying data.

120×40 cells, 1k scrollback where configurable, chunks of 8192 bytes. Seven runs per workload/engine, median wall time includes creating a fresh model and dropping it; throughput uses input bytes. No GUI, live PTY during timing, glyph rendering, font shaping, or latency. Run when machine is otherwise idle. Higher throughput here **does not mean smoother or faster terminal UI**. Different protocol feature sets and implementation paths mean this is exploratory, not a compliance-normalized comparison. The Ghostty adapter targets an older core; do not extrapolate its results to current Ghostty. `cargo test` checks **all four engines reproduce the exact expected visible text** (the `frame 0150` header and 38 numbered lines) after the same 8192-byte chunking and ten replays used in timing. Additional small-screen fixtures in `src/compat.rs` check the behaviors below with 7-byte chunking (splitting ANSI escapes and UTF-8). These checks do not establish broad VT compliance. The previous recording included a shell prompt after the last frame; it caused a scroll and divergent end state. The capture script now keeps the command running while recording the final frame, removing that ambiguous post-command transition. Replaying ten times also repeats initialization sequences; it is not a continuous tmux session.

## Core compatibility checks

| Check | Alacritty | Ghostty VT 1.2.3 | vt100 | WezTerm |
|---|:---:|:---:|:---:|:---:|
| Repeated tmux client: exact visible text | ✓ | ✓ | ✓ | ✓ |
| Cursor addressing and line clear | ✓ | ✓ | ✓ | ✓ |
| Truecolor foreground/background and bold | ✓ | ✓ | ✓ | ✓ |
| Indexed color and alternate-screen restore | ✓ | ✓ | ✓ | ✓ |
| Wide Unicode and combining marks | ✓ | ✓ | ✓ | ✓ |
| Preserve text on resize | ✓ | ✓ | ✓ | ✓ |
| Retain and inspect scrollback | ✓ | ✓ | ✓ | ✓ |

Only the **specific fixtures** pass; this is not a complete VT conformance suite. In particular, mouse input, selection, OSC clipboard, hyperlinks, grapheme clustering, error handling, exact cursor shape, rendering, and latency remain untested. The Ghostty column covers the adapted v1.2.3 wrapper, not current Ghostty.

## Recorded run (Apple M1 Pro, macOS 15.4.1, release build, 2026-10-06)

| Workload | alacritty_terminal | Ghostty VT 1.2.3 | vt100 | wezterm-term | vte parser-only |
|---|---:|---:|---:|---:|---:|
| plain scroll, ms | 19.13 | 12.69 | 9.45 | 34.98 | 1.24 |
| ANSI scroll, ms | 14.21 | 21.64 | 6.70 | 15.13 | 1.30 |
| TUI redraw, ms | 3.73 | 5.05 | 3.54 | 25.43 | 0.98 |
| tmux client (repeated recording), ms | **2.21** | 3.76 | 8.59 | 146.15 | 0.34 |

[Raw per-run medians](results/2026-10-06-m1-pro.csv) are included. Values are the **median of three independent process runs**, each reporting the median of seven iterations. This machine had noticeable variability: for example, WezTerm plain scroll ranged from 26.64–134.60 ms, and Alacritty's tmux recording from 2.19–4.49 ms. This is not a controlled system-wide benchmark.

**Do not select a GPUI terminal core on these numbers alone.** All four pass the bounded core fixtures; none is proven the fastest rendered terminal. The next meaningful decision requires broader compatibility checks and actual GPUI rendering/frame-pacing measurements. `vt100` has the best synthetic parse/state numbers here, but that alone does not establish it as the best app engine.
