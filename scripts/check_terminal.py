"""Capture actual terminal interaction in a fresh, explicitly separate UI session.

Requires pyte. Captures are evidence to inspect, not an automatic usability score.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import termios
import time

import pyte


def capture(binary, repo, output, columns, rows):
    output.mkdir(parents=True, exist_ok=False)
    session = output / "ui-events.jsonl"
    screen = pyte.Screen(columns, rows)
    stream = pyte.ByteStream(screen)
    raw_log = bytearray()

    def start():
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(repo)
            os.environ["TERM"] = "xterm-256color"
            os.execl(str(binary), str(binary), "review", "open", "--session", str(session))
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        return pid, fd

    def receive(fd, duration=0.3):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            ready, _, _ = select.select([fd], [], [], max(0, deadline - time.monotonic()))
            if not ready:
                break
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            raw_log.extend(data)
            if len(raw_log) > 2_097_152:
                raise ValueError("terminal output exceeded capture budget")
            stream.feed(data)
            if b"\x1b[6n" in data:
                os.write(fd, f"\x1b[{screen.cursor.y + 1};{screen.cursor.x + 1}R".encode())

    def snapshot(name):
        (output / (name + ".txt")).write_text("\n".join(screen.display) + "\n")

    def close(pid, fd):
        os.write(fd, b"q")
        receive(fd)
        for _ in range(20):
            result, status = os.waitpid(pid, os.WNOHANG)
            if result:
                os.close(fd)
                return os.waitstatus_to_exitcode(status)
            time.sleep(0.05)
        os.kill(pid, signal.SIGTERM)
        os.waitpid(pid, 0)
        os.close(fd)
        raise ValueError("terminal failed to quit")

    pid, fd = start()
    try:
        receive(fd, 5)
        snapshot("00-overview")
        chunks = json.loads((repo / ".gewu/review-spec.json").read_text())["chunks"]
        count = len(chunks)
        for index in range(count):
            os.write(fd, b"n")
            receive(fd)
            snapshot(f"chunk-{index + 1:02d}")
            if chunks[index].get("context"):
                os.write(fd, b"z")
                receive(fd)
                snapshot(f"context-{index + 1:02d}")
        for _ in range(max(0, count - 1)):
            os.write(fd, b"p")
            receive(fd, 0.1)
        for name, keys in [("01-first-chunk", b""), ("02-scroll", b"jjj"),
                           ("03-context", b"z"), ("04-raw-source", b"v"),
                           ("05-outline", b"o"), ("05b-close-outline", b"\x1b"), ("06-feedback-scope", b"c"),
                           ("07-feedback-editor", b"e")]:
            os.write(fd, keys)
            receive(fd)
            snapshot(name)
        os.write(fd, b"UI evaluation note: verify source navigation.\r")
        receive(fd)
        snapshot("08-feedback-saved")
        first_exit = close(pid, fd)
        pid = None
        screen.reset()
        pid, fd = start()
        receive(fd, 5)
        snapshot("09-resumed")
        second_exit = close(pid, fd)
        pid = None
    finally:
        if pid is not None:
            try:
                os.kill(pid, signal.SIGTERM)
                os.waitpid(pid, 0)
            except ProcessLookupError:
                pass
        (output / "terminal.ansi").write_bytes(raw_log)
    events = [json.loads(line) for line in session.read_text().splitlines()] if session.exists() else []
    report = {"columns": columns, "rows": rows, "exit_codes": [first_exit, second_exit],
              "fresh_session": str(session), "event_types": [event.get("type") for event in events],
              "raw_bytes": len(raw_log), "assessment": "Snapshots require visual/text inspection; no automatic quality pass."}
    (output / "capture.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--columns", type=int, required=True)
    parser.add_argument("--rows", type=int, required=True)
    args = parser.parse_args()
    capture(args.binary.resolve(), args.repo.resolve(), args.output.resolve(), args.columns, args.rows)
