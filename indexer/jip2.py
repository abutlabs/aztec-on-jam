#!/usr/bin/env python3
"""A small JIP-2 reader of our own (stdlib only): best and finalized blocks, service storage
and workPackageStatus over JSON-RPC on a WebSocket. Submissions do not go through here: the
indexer hands those to `jamboree rpc submit` (a work-package builder in Rust), so on a
jamboree net nothing of jamswap is used.
"""
import base64, json, os, socket, struct
from urllib.parse import urlparse


class Jip2Error(Exception):
    pass


class Client:
    def __init__(self, url, timeout=15.0):
        self.url, self.timeout, self.sock, self.buf, self.next_id = url, timeout, None, b"", 0

    def _connect(self):
        u = urlparse(self.url)
        s = socket.create_connection((u.hostname, u.port or 80), timeout=self.timeout)
        key = base64.b64encode(os.urandom(16)).decode()
        s.sendall((f"GET {u.path or '/'} HTTP/1.1\r\nHost: {u.hostname}:{u.port}\r\nUpgrade: websocket\r\n"
                   f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = s.recv(4096)
            if not chunk:
                raise Jip2Error("the node closed the connection during the handshake")
            head += chunk
        status = head.split(b"\r\n", 1)[0]
        if b" 101 " not in status:
            raise Jip2Error("no WebSocket upgrade: %r" % status)
        self.sock, self.buf = s, head.split(b"\r\n\r\n", 1)[1]

    def _send(self, text):
        data = text.encode(); mask = os.urandom(4)
        n = len(data)
        hdr = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n)
                               if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.sock.sendall(hdr + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def _take(self, n):
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise Jip2Error("the node closed the connection")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def _recv(self):
        parts = []
        while True:
            b0, b1 = self._take(2)
            n = b1 & 0x7F
            n = struct.unpack(">H", self._take(2))[0] if n == 126 else struct.unpack(">Q", self._take(8))[0] if n == 127 else n
            mask = self._take(4) if b1 & 0x80 else None
            data = self._take(n)
            if mask:
                data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
            op = b0 & 0x0F
            if op == 0x8:
                raise Jip2Error("the node closed the WebSocket")
            if op == 0x9:                        # ping: answer it
                self.sock.sendall(bytes([0x8A, 0x80]) + os.urandom(4)); continue
            if op in (0x1, 0x2, 0x0):
                parts.append(data)
                if b0 & 0x80:
                    return b"".join(parts).decode()

    def call(self, method, *params):
        for attempt in (0, 1):
            try:
                if self.sock is None:
                    self._connect()
                self.next_id += 1
                rid = self.next_id
                self._send(json.dumps({"jsonrpc": "2.0", "id": rid, "method": method, "params": list(params)}))
                while True:
                    msg = json.loads(self._recv())
                    if isinstance(msg, dict) and msg.get("id") in (rid, None):
                        break
            except (OSError, ValueError, Jip2Error):
                self.sock = None
                if attempt:
                    raise
                continue
            if msg.get("error"):
                raise Jip2Error(f"{method}: {msg['error']}")
            return msg.get("result")


b64 = lambda b: base64.b64encode(bytes(b)).decode()  # noqa: E731
unb64 = lambda s: base64.b64decode(s)  # noqa: E731


class Chain:
    """What the indexer reads: blocks, one service's storage, a package's status."""
    def __init__(self, url):
        self.rpc = Client(url)

    def _block(self, method):
        d = self.rpc.call(method)
        return int(d["slot"]), unb64(d["header_hash"])

    def head(self):
        return self._block("bestBlock")

    def finalized(self):
        return self._block("finalizedBlock")

    def parent(self, at):
        """The parent of block `at`: (slot, hash), or None for genesis or a pruned block."""
        d = self.rpc.call("parent", b64(at))
        return (int(d["slot"]), unb64(d["header_hash"])) if d else None

    def read(self, service, key, at=None):
        at = at or self.head()[1]
        v = self.rpc.call("serviceValue", b64(at), service, b64(key))
        return unb64(v) if v else b""

    def package_status(self, package_hex, anchor_hex):
        at = self.head()[1]
        return self.rpc.call("workPackageStatus", b64(at), b64(bytes.fromhex(package_hex)), b64(bytes.fromhex(anchor_hex)))
