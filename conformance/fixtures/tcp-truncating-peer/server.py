"""Malformed loopback peer: sends a fragment without a terminator and closes immediately."""
import socket

srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0))
srv.listen(8)
print("READY port=%d" % srv.getsockname()[1], flush=True)
while True:
    conn, _ = srv.accept()
    conn.sendall(b"partial")
    conn.close()
