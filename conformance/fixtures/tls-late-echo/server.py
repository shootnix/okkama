"""Loopback TLS fixture that answers late.

Binds 127.0.0.1 on an OS-assigned port and announces it as `READY port=N`.
For each connection: TLS handshake with the test certificate next to this
file (self-signed, IP:127.0.0.1, test-only); then it stays silent until the
client sends one byte, answers with the 5 bytes `late!` and reads until the
client closes. The client interrupts its reads (a timeout, a cancelled task)
while the fixture is silent and only then sends the byte, so the record
arrives after the interruptions however slowly the client runs (#312: a
fixed pause lost the race to a loaded machine compiling the client).
Owned by the runner: it is terminated when the case finishes.
"""
import os
import socket
import ssl
import sys
import threading

HERE = os.path.dirname(os.path.abspath(__file__))
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(os.path.join(HERE, "cert.pem"), os.path.join(HERE, "key.pem"))

srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0))
srv.listen(8)
print("READY port=%d" % srv.getsockname()[1], flush=True)


def serve(raw):
    try:
        conn = context.wrap_socket(raw, server_side=True)
        conn.settimeout(60)
        conn.recv(1)
        conn.sendall(b"late!")
        while conn.recv(4096):
            pass
        conn.close()
        sys.stderr.write("served\n")
    except Exception as error:  # noqa: BLE001 - the log is the diagnostic
        sys.stderr.write("connection failed: %r\n" % (error,))
    sys.stderr.flush()


while True:
    raw, _ = srv.accept()
    threading.Thread(target=serve, args=(raw,), daemon=True).start()
