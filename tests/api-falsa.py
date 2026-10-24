"""Doble de pruebas de la API REST de Iurefficient (login, TOTP, refresh, me, WebDAV)."""
import json, sys, secrets
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

USER = {"id": 1, "email": "ana@despacho.com", "name": "Ana Torres"}
TOTP = "--totp" in sys.argv
sessions = set()
pending = {}

class H(BaseHTTPRequestHandler):
    def _json(self, code, body, cookies=()):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        for c in cookies:
            self.send_header("Set-Cookie", c)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def _cookie(self, name):
        for part in (self.headers.get("Cookie") or "").split(";"):
            k, _, v = part.strip().partition("=")
            if k == name:
                return v
        return None

    def _issue(self):
        tok = secrets.token_hex(8); ref = secrets.token_hex(8)
        sessions.add(tok); sessions.add("r" + ref)
        return [f"access_token_cookie={tok}; Path=/api/; HttpOnly",
                f"refresh_token_cookie=r{ref}; Path=/api/auth/refresh; HttpOnly",
                f"csrf_access_token=c{tok}; Path=/", f"csrf_refresh_token=c{ref}; Path=/"]

    def log_message(self, *a):
        sys.stderr.write("fake: " + (a[0] % a[1:]) + "\n")

    def do_POST(self):
        b = self._body()
        if self.path == "/api/auth/login":
            if b.get("email") != USER["email"] or b.get("password") != "secreta":
                return self._json(401, {"error": "bad"})
            if TOTP:
                t = secrets.token_hex(6); pending[t] = True
                return self._json(200, {"requires_totp": True, "totp_token": t})
            return self._json(200, {"user": USER}, self._issue())
        if self.path == "/api/auth/verify-totp":
            if pending.pop(b.get("totp_token"), None) and b.get("code") == "123456":
                return self._json(200, {"user": USER}, self._issue())
            return self._json(401, {"error": "code"})
        if self.path == "/api/auth/refresh":
            if self._cookie("refresh_token_cookie") in sessions:
                return self._json(200, {"ok": True}, self._issue())
            return self._json(401, {"error": "expired"})
        if self.path == "/api/system/webdav/tokens":
            if self._cookie("access_token_cookie") not in sessions:
                return self._json(401, {"error": "auth"})
            return self._json(201, {"secret": "iurdav_" + secrets.token_hex(8), "token": {"id": "t1", "name": b.get("name")}, "username": USER["email"]})
        if self.path == "/api/auth/logout":
            sessions.discard(self._cookie("access_token_cookie"))
            return self._json(200, {"ok": True})
        self._json(404, {})

    def do_GET(self):
        if self.path == "/api/auth/me":
            if self._cookie("access_token_cookie") in sessions:
                return self._json(200, {"user": USER})
            return self._json(401, {"error": "auth"})
        self._json(404, {})

ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
