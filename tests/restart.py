"""Run after cargo build: python3 tests/restart.py (Unix PTY, no model calls)."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time
import tempfile
from builder import command

ROOT = Path(__file__).resolve().parents[1]


def launch(project, env):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 100, 0, 0))
    process = subprocess.Popen(
        [str(ROOT / "target/debug/tui-draw"), "chat", str(project)],
        stdin=slave, stdout=slave, stderr=slave,
        env={**env, "TERM": "xterm-256color"},
    )
    os.close(slave)
    output = bytearray()
    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and b"Ctrl+C Quit" not in output:
            if select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
        alternate = output.index(b"\x1b[?1049h")
        first_frame = output.index(b"TUI Draw")
        assert b"\x1b[2J" in output[alternate:first_frame], (
            "Startup must erase physical alternate-screen cells before drawing; "
            "blank cells in Ratatui's new diff buffers cannot erase a previous run."
        )
        os.write(master, b"\x03")
        assert process.wait(timeout=3) == 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="tui-restart-") as folder:
        env = {**os.environ, "XDG_STATE_HOME": str(Path(folder) / "state")}
        for name in ["calculator", "dungeon-scene"]:
            project = Path(folder) / name
            command("build", "--spec", ROOT / f"examples/{name}.json", "--out", project, env=env)
            launch(project, env)
    print("PASS: calculator and dungeon launches clear the alternate screen before drawing")
