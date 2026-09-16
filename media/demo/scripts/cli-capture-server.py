#!/usr/bin/env python3
"""Capture a real interactive CLI next to a real Conn browser frontend.

Linux/macOS only; Python's standard library and the frontend's installed xterm
assets are sufficient. The configured child starts once, not per browser visit.
Only its PTY can receive input/resize events. No HTTP command-execution endpoint
exists. Bind is always loopback; WebSocket requests require a same-origin Origin.

Example:
  python3 cli-capture-server.py --cwd /tmp/conn-film-v2/workspace \
    --record /tmp/conn-film-v2/codex-pty.jsonl -- codex --no-alt-screen

The private JSONL log contains timestamped actual PTY output, base64 encoded.
Do not publish it without reviewing the child's output for personal information.
Environment variables, authentication/config files, input, and CLI arguments are
not dumped. Browser refresh replays up to --replay-bytes of existing output.
"""

import argparse
import base64
import collections
import fcntl
import hashlib
import http.client
import http.server
import json
import os
from pathlib import Path
import pty
import select
import signal
import socket
import struct
import sys
import termios
import threading
import time
from urllib.parse import unquote, urlsplit


WS_MAGIC = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
MAX_MESSAGE = 65536


def ws_frame(opcode, data):
    length = len(data)
    if length < 126:
        header = bytes([0x80 | opcode, length])
    elif length < 65536:
        header = bytes([0x80 | opcode, 126]) + struct.pack("!H", length)
    else:
        header = bytes([0x80 | opcode, 127]) + struct.pack("!Q", length)
    return header + data


class Peer:
    def __init__(self, connection):
        self.connection = connection
        self.lock = threading.Lock()

    def send(self, opcode, data):
        with self.lock:
            self.connection.sendall(ws_frame(opcode, data))

    def message(self, value):
        self.send(1, json.dumps(value, separators=(",", ":")).encode())


class Session:
    def __init__(self, args):
        self.args = args
        self.lock = threading.RLock()
        self.peers = set()
        self.output = collections.deque()
        self.output_bytes = 0
        self.exit_code = None
        self.closed = False
        self.started_at = time.monotonic()
        self.log = None
        if args.record:
            args.record.parent.mkdir(parents=True, exist_ok=True)
            descriptor = os.open(args.record, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            self.log = os.fdopen(descriptor, "w", encoding="utf-8")
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            try:
                os.chdir(args.cwd)
                os.environ["TERM"] = "xterm-256color"
                os.environ.setdefault("COLORTERM", "truecolor")
                os.execvp(args.command[0], args.command)
            except Exception as error:
                print(f"Could not start CLI: {error}", file=sys.stderr, flush=True)
                os._exit(127)
        self.resize(80, 36)
        self.record("start", executable=Path(args.command[0]).name)
        self.reader = threading.Thread(target=self.read_output, daemon=True)
        self.reader.start()

    def record(self, kind, **values):
        if self.log:
            self.log.write(json.dumps({"event": kind, "unixTime": time.time(),
                                      "elapsed": time.monotonic() - self.started_at,
                                      **values}, separators=(",", ":")) + "\n")
            self.log.flush()

    def broadcast(self, data):
        with self.lock:
            self.output.append(data)
            self.output_bytes += len(data)
            while self.output_bytes > self.args.replay_bytes and self.output:
                self.output_bytes -= len(self.output.popleft())
            for peer in tuple(self.peers):
                try:
                    peer.send(2, data)
                except OSError:
                    self.peers.discard(peer)

    def attach(self, peer):
        with self.lock:
            peer.message({"type": "session", "alive": self.exit_code is None,
                          "exitCode": self.exit_code})
            # Preserve output order: catch-up is sent under the broadcast lock.
            for chunk in self.output:
                peer.send(2, chunk)
            self.peers.add(peer)

    def detach(self, peer):
        with self.lock:
            self.peers.discard(peer)

    def read_output(self):
        while not self.closed:
            try:
                readable, _, _ = select.select([self.fd], [], [], 0.3)
                if not readable:
                    continue
                data = os.read(self.fd, 65536)
                if not data:
                    break
            except OSError:
                break
            with self.lock:
                self.record("output", data=base64.b64encode(data).decode("ascii"))
            self.broadcast(data)
        try:
            _, status = os.waitpid(self.pid, 0)
            code = os.waitstatus_to_exitcode(status)
        except ChildProcessError:
            code = 0
        with self.lock:
            self.exit_code = code
            self.record("exit", code=code)
            for peer in tuple(self.peers):
                try:
                    peer.message({"type": "exit", "code": code})
                except OSError:
                    self.peers.discard(peer)

    def write(self, data):
        if self.exit_code is not None or self.closed:
            return
        encoded = data.encode("utf-8")
        while encoded:
            written = os.write(self.fd, encoded)
            encoded = encoded[written:]

    def resize(self, cols, rows):
        if isinstance(cols, bool) or isinstance(rows, bool):
            raise ValueError("Invalid PTY dimensions")
        if not isinstance(cols, int) or not isinstance(rows, int):
            raise ValueError("Invalid PTY dimensions")
        if not 2 <= cols <= 500 or not 2 <= rows <= 200:
            raise ValueError("PTY dimensions out of range")
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        with self.lock:
            self.record("resize", cols=cols, rows=rows)

    def close(self):
        self.closed = True
        if self.exit_code is None:
            try:
                os.killpg(self.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            self.reader.join(timeout=2)
            if self.reader.is_alive():
                try:
                    os.killpg(self.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.reader.join(timeout=2)
        try:
            os.close(self.fd)
        except OSError:
            pass
        with self.lock:
            for peer in tuple(self.peers):
                try:
                    peer.connection.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
            if self.log:
                self.log.close()
                self.log = None


def make_handler(session, args):
    script_dir = Path(__file__).resolve().parent
    assets = {
        "/xterm.js": (args.xterm_root / "@xterm/xterm/lib/xterm.js", "text/javascript"),
        "/addon-fit.js": (args.xterm_root / "@xterm/addon-fit/lib/addon-fit.js", "text/javascript"),
        "/xterm.css": (args.xterm_root / "@xterm/xterm/css/xterm.css", "text/css"),
    }
    allowed_origins = {f"http://127.0.0.1:{args.port}", f"http://localhost:{args.port}"}
    conn_url = args.conn_url.rstrip("/") + "/"
    conn_target = urlsplit(conn_url)
    conn_origin = f"{conn_target.scheme}://{conn_target.netloc}"
    iframe_url = "/conn/" if args.proxy_conn else conn_url
    frame_source = "'self'" if args.proxy_conn else conn_origin

    def proxy_path_allowed(path):
        decoded = unquote(path)
        # Vite's filesystem/editor endpoints are intentionally unavailable.
        if any(char in decoded for char in ("%", "\\", "\x00")) or ".." in decoded.split("/"):
            return False
        return (decoded in {"/conn/", "/__conn/connection", "/conn-icon.svg", "/favicon.ico"}
                or decoded.startswith(("/src/", "/node_modules/", "/@vite/", "/@id/", "/tests/browser/")))

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, format_string, *values):
            # No URL/query payload logging; only the child PTY is recorded.
            return

        def do_GET(self):
            path = urlsplit(self.path).path
            if self.headers.get("Host") not in {f"127.0.0.1:{args.port}", f"localhost:{args.port}"}:
                self.send_error(403, "Unexpected Host")
                return
            if path == "/terminal":
                self.websocket()
                return
            if args.proxy_conn and proxy_path_allowed(path):
                self.proxy_conn(path)
                return
            if path == "/":
                data = (script_dir / "capture-shell.html").read_text(encoding="utf-8")
                data = data.replace("__CONN_URL__", iframe_url)
                body, content_type = data.encode(), "text/html; charset=utf-8"
            elif path in assets:
                asset, content_type = assets[path]
                body = asset.read_bytes()
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'self'; "
                             "script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; "
                             f"connect-src 'self'; frame-src {frame_source}; "
                             "frame-ancestors 'none'; base-uri 'none'")
            self.end_headers()
            self.wfile.write(body)

        def proxy_conn(self, path):
            origin = self.headers.get("Origin")
            if origin and origin not in allowed_origins:
                self.send_error(403, "A same-origin browser is required")
                return
            # Fixed, validated loopback target. Never follow redirects or forward
            # request credentials/cookies/authorization to the development server.
            query = urlsplit(self.path).query
            target_path = "/" if path == "/conn/" else path
            if query:
                target_path += "?" + query
            upstream = http.client.HTTPConnection(conn_target.hostname, conn_target.port or 80, timeout=10)
            try:
                upstream.request("GET", target_path, headers={"Accept-Encoding": "identity"})
                response = upstream.getresponse()
                if 300 <= response.status < 400:
                    self.send_error(502, "Unexpected development server redirect")
                    return
                body = response.read(32 * 1024 * 1024 + 1)
                if len(body) > 32 * 1024 * 1024:
                    self.send_error(502, "Development asset too large")
                    return
                self.send_response(response.status)
                self.send_header("Content-Type", response.getheader("Content-Type", "application/octet-stream"))
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Cache-Control", "no-store")
                self.send_header("X-Content-Type-Options", "nosniff")
                self.send_header("Cross-Origin-Resource-Policy", "same-origin")
                self.send_header("Content-Security-Policy", "frame-ancestors 'self'")
                self.end_headers()
                self.wfile.write(body)
            except (OSError, http.client.HTTPException):
                self.send_error(502, "Conn development server unavailable")
            finally:
                upstream.close()

        def websocket(self):
            if self.headers.get("Origin") not in allowed_origins:
                self.send_error(403, "A same-origin browser is required")
                return
            if (self.headers.get("Upgrade", "").lower() != "websocket"
                    or "upgrade" not in self.headers.get("Connection", "").lower()
                    or self.headers.get("Sec-WebSocket-Version") != "13"):
                self.send_error(400, "Expected a WebSocket upgrade")
                return
            key = self.headers.get("Sec-WebSocket-Key", "")
            try:
                if len(base64.b64decode(key, validate=True)) != 16:
                    raise ValueError("Invalid key")
            except ValueError:
                self.send_error(400, "Invalid WebSocket key")
                return
            accepted = base64.b64encode(hashlib.sha1((key + WS_MAGIC).encode()).digest()).decode()
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.send_header("Sec-WebSocket-Accept", accepted)
            self.end_headers()
            self.close_connection = True
            peer = Peer(self.connection)
            self.connection.settimeout(5)
            try:
                session.attach(peer)
                self.connection.settimeout(None)
                while True:
                    header = self.read_exact(2)
                    if header is None:
                        break
                    final, opcode = bool(header[0] & 0x80), header[0] & 15
                    if header[0] & 0x70 or not final or not header[1] & 0x80:
                        raise ValueError("Unsupported WebSocket frame")
                    length = header[1] & 127
                    if length == 126:
                        length = struct.unpack("!H", self.read_exact(2))[0]
                    elif length == 127:
                        length = struct.unpack("!Q", self.read_exact(8))[0]
                    if length > MAX_MESSAGE or (opcode >= 8 and length > 125):
                        raise ValueError("Message too large")
                    mask = self.read_exact(4)
                    payload = self.read_exact(length)
                    if mask is None or payload is None:
                        break
                    payload = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
                    if opcode == 8:
                        peer.send(8, payload)
                        break
                    if opcode == 9:
                        peer.send(10, payload)
                        continue
                    if opcode == 10:
                        continue
                    if opcode != 1:
                        raise ValueError("Expected a text control message")
                    message = json.loads(payload.decode("utf-8"))
                    if not isinstance(message, dict):
                        raise ValueError("Expected an object")
                    if message.get("type") == "input" and isinstance(message.get("data"), str):
                        session.write(message["data"])
                    elif message.get("type") == "resize":
                        session.resize(message.get("cols"), message.get("rows"))
                    else:
                        raise ValueError("Unsupported terminal operation")
            except (OSError, ValueError, TypeError, struct.error):
                pass
            finally:
                session.detach(peer)

        def read_exact(self, length):
            output = bytearray()
            while len(output) < length:
                chunk = self.rfile.read(length - len(output))
                if not chunk:
                    return None
                output.extend(chunk)
            return bytes(output)

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=1435)
    parser.add_argument("--cwd", type=Path, required=True)
    parser.add_argument("--conn-url", default="http://127.0.0.1:1431/")
    parser.add_argument("--proxy-conn", action="store_true",
                        help="Serve Conn's real Vite frontend under /conn/ on this origin; configure the Rust backend Origin accordingly")
    parser.add_argument("--record", type=Path)
    parser.add_argument("--replay-bytes", type=int, default=2 * 1024 * 1024)
    parser.add_argument("--xterm-root", type=Path,
                        default=Path(__file__).resolve().parents[3] / "frontends/tauri/node_modules")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("Supply an executable and arguments after --")
    if not args.cwd.is_dir():
        parser.error("--cwd must be an existing directory")
    if not 1024 <= args.port <= 65535 or args.replay_bytes < 65536:
        parser.error("Use an unprivileged port and at least 65536 replay bytes")
    conn = urlsplit(args.conn_url)
    if (conn.scheme != "http" or conn.hostname not in {"127.0.0.1", "localhost"}
            or conn.username or conn.password or conn.query or conn.fragment
            or conn.path not in {"", "/"}):
        parser.error("--conn-url must be a loopback http origin")
    for file in ["@xterm/xterm/lib/xterm.js", "@xterm/xterm/css/xterm.css",
                 "@xterm/addon-fit/lib/addon-fit.js"]:
        if not (args.xterm_root / file).is_file():
            parser.error(f"Missing xterm asset: {file}; install frontend dependencies")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), http.server.BaseHTTPRequestHandler)
    server.daemon_threads = True
    session = None
    try:
        session = Session(args)
        server.RequestHandlerClass = make_handler(session, args)

        def stop(_signum, _frame):
            threading.Thread(target=server.shutdown, daemon=True).start()

        signal.signal(signal.SIGINT, stop)
        signal.signal(signal.SIGTERM, stop)
        print(f"Capture bridge: http://127.0.0.1:{args.port}/ (child PID {session.pid})", flush=True)
        server.serve_forever(poll_interval=0.25)
    finally:
        server.server_close()
        if session:
            session.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
