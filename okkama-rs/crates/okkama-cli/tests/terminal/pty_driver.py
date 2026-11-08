"""Run an Okkama program under a pseudo-terminal and script it (#12).

    python3 pty_driver.py <okkama> <engine> <program.ok>

The program (terminal/cancel.ok) reads the controlling terminal. Its first
two reads are interrupted (a timeout, a cancelled task) before any input
exists; only after it prints "waiting for a line" does the driver type
`abc`, which the next read must get whole. After "waiting for a resize" the
driver changes the window to 100x40, which the program must report. Exit 0
when the transcript holds every expected line and the program exits 0.
"""
import fcntl
import os
import pty
import select
import struct
import sys
import termios
import time

okkama, engine, program = sys.argv[1:4]
EXPECTED = ["timed out", "cancelled", "line: abc", "initial: 80x24", "size: 100x40"]


def winsize(fd, rows, columns):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))


pid, master = pty.fork()
if pid == 0:
    os.execv(okkama, [okkama, "run", "--engine", engine, program])
winsize(master, 24, 80)
transcript = b""
deadline = time.monotonic() + 20
typed = resized = False
status = None
while time.monotonic() < deadline:
    ready, _, _ = select.select([master], [], [], 0.1)
    if ready:
        try:
            chunk = os.read(master, 4096)
        except OSError:
            chunk = b""
        if not chunk:
            break
        transcript += chunk
    if not typed and b"waiting for a line" in transcript:
        os.write(master, b"abc\n")
        typed = True
    if not resized and b"waiting for a resize" in transcript:
        time.sleep(0.2)
        winsize(master, 40, 100)
        resized = True
    done, exited = os.waitpid(pid, os.WNOHANG)
    if done:
        status = exited
        break
# What the program wrote just before it exited is still in the pty.
while True:
    ready, _, _ = select.select([master], [], [], 0.2)
    if not ready:
        break
    try:
        chunk = os.read(master, 4096)
    except OSError:
        break
    if not chunk:
        break
    transcript += chunk
if status is None or not os.WIFEXITED(status):
    try:
        os.kill(pid, 9)
    except ProcessLookupError:
        pass
    _, status = os.waitpid(pid, 0)
text = transcript.decode("utf-8", "replace").replace("\r\n", "\n")
code = os.WEXITSTATUS(status) if os.WIFEXITED(status) else -1
missing = [line for line in EXPECTED if line not in text]
sys.stdout.write(text)
if code != 0 or missing:
    sys.stderr.write("engine %s: exit %s, missing %s\n" % (engine, code, missing))
    sys.exit(1)
