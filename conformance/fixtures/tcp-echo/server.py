"""Loopback TCP echo fixture: replies b"echo:" + one LF-terminated line, then closes.

Binds 127.0.0.1 on an OS-assigned port and announces it as `READY port=N`.
Owned by the runner: it is terminated when the case finishes.
"""
import socket
import sys

srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0))
srv.listen(8)
print("READY port=%d" % srv.getsockname()[1], flush=True)
while True:
    conn, _ = srv.accept()
    data = b""
    conn.settimeout(10)
    try:
        while not data.endswith(b"\n"):
            chunk = conn.recv(4096)
            if not chunk:
                break
            data += chunk
        conn.sendall(b"echo:" + data)
    finally:
        conn.close()
    sys.stderr.write("served %d bytes\n" % len(data))
    sys.stderr.flush()
