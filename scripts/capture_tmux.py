#!/usr/bin/env python3
"""Capture bytes emitted by a real tmux client into a 120x40 PTY."""
import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import termios
import time
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
out = root / "fixtures" / "tmux-redraw.bin"
out.parent.mkdir(exist_ok=True)
name = f"vtbench-{os.getpid()}"
tmux = os.environ.get("TMUX_BIN", "tmux")
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
env = {**os.environ, "TERM": "xterm-256color", "TMUX": "", "ENV": ""}
config = tempfile.NamedTemporaryFile(mode="w", prefix="vtbench-tmux-", delete=False)
config.write("set -g status off\nset -g default-shell /bin/sh\nset -g default-command /bin/sh\n")
config.close()
client = subprocess.Popen(
    [tmux, "-L", name, "-f", config.name, "new-session", "-s", "bench", "-x", "120", "-y", "40"],
    stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True,
)
os.close(slave)
data = bytearray()
try:
    # Allow server startup, send a series of full-pane repaints, and drain the PTY.
    time.sleep(0.5)
    command = "for i in $(seq 1 150); do printf '\\033[H\\033[2Jframe %04d\\n' \"$i\"; seq 1 38; sleep 0.005; done"
    subprocess.run([tmux, "-L", name, "send-keys", "-t", "bench", "-l", command], check=True)
    subprocess.run([tmux, "-L", name, "send-keys", "-t", "bench", "Enter"], check=True)
    end = time.monotonic() + 8
    while time.monotonic() < end:
        if select.select([master], [], [], 0.1)[0]:
            try:
                data.extend(os.read(master, 65536))
            except OSError:
                break
    if not data or b"frame " not in data:
        raise RuntimeError("tmux produced no rendered frames")
    out.write_bytes(data)
    print(f"Captured {len(data)} bytes to {out}")
finally:
    subprocess.run([tmux, "-L", name, "kill-server"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.killpg(client.pid, signal.SIGTERM)
    client.wait(timeout=5)
    os.close(master)
    os.unlink(config.name)
