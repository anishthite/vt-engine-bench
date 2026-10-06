# VT engine benchmark (macOS)

Headless **parsing + screen-state update**, not terminal-rendering speed. Sources are pinned as git submodules (see `git submodule status` and `Cargo.lock`). Ghostty uses the Apache-2.0 Zig/C ABI + Rust wrapper adapted from [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty), against Ghostty v1.2.3. The other engines are recent source checkouts. `vte` has **no screen model** and is included as an explicitly incomparable parser-only baseline.

```sh
git submodule update --init --recursive
./bootstrap-zig.sh # Zig 0.14.1, only if not installed
cargo run --release # or ZIG=/path/to/zig cargo run --release
```

Three workloads are generated in `src/main.rs`: 15k plain scroll lines, 15k 256-color lines, and 10k cursor-positioned/cleared TUI updates. A fourth is a **real tmux client PTY recording** (`fixtures/tmux-redraw.bin`, 25,455 bytes, SHA-256 `2a67411cae71c8faffb957a095aec79c6c5e63975a33bf82adf0fe1e5b4bb67a`) captured by `python3 scripts/capture_tmux.py`, replayed ten times for timing. Re-capturing changes the fixture; the committed bytes define this run. The capture uses `/bin/sh`, a temporary socket and config, and disables tmux's status line to avoid machine-identifying data.

120×40 cells, 1k scrollback where configurable, chunks of 8192 bytes. Seven runs per workload/engine, median wall time includes creating a fresh model and dropping it; throughput uses input bytes. No GUI, live PTY during timing, glyph rendering, font shaping, or latency. Run when machine is otherwise idle. Higher throughput here **does not mean smoother or faster terminal UI**. Different protocol feature sets and implementation paths mean this is exploratory, not a compliance-normalized comparison. The Ghostty adapter targets an older core; do not extrapolate its results to current Ghostty. `cargo test` checks the tmux fixture produces visible output in each engine; it does **not** assert identical viewports: the engines diverge on the final screen (Ghostty retains `frame 0150` while Alacritty's viewport starts with `1`). The repeated tmux recording also repeats initialization/teardown sequences, so it is not a continuous tmux session.

## Recorded run (Apple M1 Pro, macOS 15.4.1, release build, 2026-10-06)

| Workload | alacritty_terminal | Ghostty VT 1.2.3 | vt100 | wezterm-term | vte parser-only |
|---|---:|---:|---:|---:|---:|
| plain scroll, ms | 13.01 | 9.65 | 8.29 | 26.27 | 0.73 |
| ANSI scroll, ms | 12.38 | 17.17 | 6.15 | 14.29 | 1.29 |
| TUI redraw, ms | 3.72 | 5.03 | 3.54 | 26.81 | 0.93 |
| tmux client (repeated recording), ms | **2.14** | 3.61 | 8.61 | 152.43 | 0.33 |

**Do not select a GPUI terminal core on these numbers alone.** Next steps before a production choice: validate rendered cells against common fixtures (including tmux output), update Ghostty to a current API, and benchmark actual GPUI rendering/frame pacing under tmux. The `vt100` result particularly needs compatibility evaluation before comparing as an app engine.
