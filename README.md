# VT engine benchmark (macOS)

Headless **parsing + screen-state update**, not terminal-rendering speed. Sources are pinned as git submodules (see `git submodule status` and `Cargo.lock`). Ghostty uses the Apache-2.0 Zig/C ABI + Rust wrapper adapted from [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty), against Ghostty v1.2.3. The other engines are recent source checkouts. `vte` has **no screen model** and is included as an explicitly incomparable parser-only baseline.

```sh
git submodule update --init --recursive
./bootstrap-zig.sh # Zig 0.14.1, only if not installed
cargo run --release # or ZIG=/path/to/zig cargo run --release
```

Three workloads are generated in `src/main.rs`: 15k plain scroll lines, 15k 256-color lines, and 10k cursor-positioned/cleared TUI updates. A fourth is a **real tmux client PTY recording** (`fixtures/tmux-redraw.bin`, 25,641 bytes, SHA-256 `d30383bb7658ae386f6f8e70e7d26bc8f1484b95d1199cbaf81715ad7233b7da`) captured by `python3 scripts/capture_tmux.py`, replayed ten times for timing. Re-capturing changes the fixture; the committed bytes define this run. The capture uses `/bin/sh`, a temporary socket and config, and disables tmux's status line to avoid machine-identifying data.

120×40 cells, 1k scrollback where configurable, chunks of 8192 bytes. Seven runs per workload/engine, median wall time includes creating a fresh model and dropping it; throughput uses input bytes. No GUI, live PTY during timing, glyph rendering, font shaping, or latency. Run when machine is otherwise idle. Higher throughput here **does not mean smoother or faster terminal UI**. Different protocol feature sets and implementation paths mean this is exploratory, not a compliance-normalized comparison. The Ghostty adapter targets an older core; do not extrapolate its results to current Ghostty. `cargo test` checks **all four engines reproduce the exact expected visible text** (the `frame 0150` header and 38 numbered lines) after the same 8192-byte chunking and ten replays used in timing. This does not verify colors, cursor, attributes, or broader VT compliance. The previous recording included a shell prompt after the last frame; it caused a scroll and divergent end state. The capture script now keeps the command running while recording the final frame, removing that ambiguous post-command transition. Replaying ten times also repeats initialization sequences; it is not a continuous tmux session.

## Recorded run (Apple M1 Pro, macOS 15.4.1, release build, 2026-10-06)

| Workload | alacritty_terminal | Ghostty VT 1.2.3 | vt100 | wezterm-term | vte parser-only |
|---|---:|---:|---:|---:|---:|
| plain scroll, ms | 14.69 | 13.30 | 8.57 | 32.20 | 0.73 |
| ANSI scroll, ms | 11.96 | 16.93 | 6.57 | 14.28 | 1.29 |
| TUI redraw, ms | 3.76 | 5.24 | 3.59 | 25.77 | 0.94 |
| tmux client (repeated recording), ms | **2.16** | 3.62 | 8.48 | 141.65 | 0.36 |

**Do not select a GPUI terminal core on these numbers alone.** Next steps before a production choice: validate cell colors/attributes and additional tmux behaviors, update Ghostty to a current API, and benchmark actual GPUI rendering/frame pacing under tmux. The `vt100` result particularly needs compatibility evaluation before comparing as an app engine.
