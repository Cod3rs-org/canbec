"""Serves the fictional demo sites in test/fixtures/demo by host name, for screenshots:
python3 tools/demo_server.py [port]   (port 80 needs root; point the names at 127.0.0.1 first:
journal.example garden.example aiboost.example fieldkit.example in /etc/hosts)"""
import http.server, os, sys

ROOT = os.path.join(os.path.dirname(__file__), "..", "test", "fixtures", "demo")
SITES = {"journal.example": "journal.html", "garden.example": "garden.html",
         "aiboost.example": "aiboost.html", "fieldkit.example": "fieldkit.html"}


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        page = SITES.get(self.headers.get("Host", "").split(":")[0])
        if not page or self.path.split("?")[0] != "/":
            self.send_error(404)
            return
        body = open(os.path.join(ROOT, page), "rb").read()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 80), Handler).serve_forever()
