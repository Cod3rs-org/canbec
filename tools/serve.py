"""Serve a static folder for engine tests: python3 tools/serve.py <dir> [port].
ponytail: stdlib server; its default backlog (5) drops Blazor's parallel .wasm downloads, so raise it."""
import functools, http.server, sys

class Server(http.server.ThreadingHTTPServer):
    request_queue_size = 256

directory, port = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 5080
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
Server(("127.0.0.1", port), handler).serve_forever()
