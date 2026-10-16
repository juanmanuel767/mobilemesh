#!/usr/bin/env python3
"""Sirve esta carpeta con las cabeceras que necesita el SDK web (memoria compartida).
Uso: python3 servir.py [puerto]"""
import http.server, sys

class Manejador(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()

Manejador.extensions_map[".js"] = "text/javascript"
Manejador.extensions_map[".wasm"] = "application/wasm"
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 8080), Manejador).serve_forever()
