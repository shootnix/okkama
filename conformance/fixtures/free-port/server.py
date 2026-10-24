"""Announces a loopback port that had a listener which is now closed (connection refused target)."""
import socket
import time

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.bind(("127.0.0.1", 0))
port = s.getsockname()[1]
s.close()
print("READY port=%d" % port, flush=True)
while True:
    time.sleep(3600)
