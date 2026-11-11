#!/usr/bin/env python3
"""A9 — a server that refuses every token: any request (the WebSocket
upgrade, the solo-login discovery, the failure probe) gets 401. The Connect
walk points the app at it to prove the refused-token path without touching a
real server.

  python3 tools/walk/a9_auth401_serve.py 8439
"""
import http.server
import socketserver
import sys


class Refuse(http.server.BaseHTTPRequestHandler):
    def _refuse(self):
        body = b"unauthorized"
        self.send_response(401)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_GET = _refuse
    do_POST = _refuse

    def log_message(self, fmt, *args):
        sys.stdout.write("[auth401] " + (fmt % args) + "\n")
        sys.stdout.flush()


class Server(http.server.ThreadingHTTPServer):
    def server_bind(self):
        # No reverse lookup of the loopback address: with no DNS at hand it
        # outlasts the walk's wait for this port.
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8439
    Server(("127.0.0.1", port), Refuse).serve_forever()
