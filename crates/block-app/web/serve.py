#!/usr/bin/env python3
#
# The web bundle and the server behind it, on this machine:
#
#   ./scripts/buck run //crates/block-app:web-serve
#
# Starts block-server on --backend and serves the bundle on --listen: files
# with the two headers that make the page cross-origin isolated, which the
# module's shared memory needs before a browser will hand it out, and /api
# passed through to the server byte for byte, which is what lets the app's
# WebSocket upgrade through it. web/Caddyfile is the same arrangement for a
# deployment, with TLS.
#
# Anything after the options is handed to block-server; the default is
# --disable-registration, as a deployment runs it.

import argparse
import asyncio
import mimetypes
import os
import subprocess
import sys
from urllib.parse import unquote, urlsplit

HEADERS = (
    b"Cross-Origin-Opener-Policy: same-origin\r\n"
    b"Cross-Origin-Embedder-Policy: require-corp\r\n"
    b"Cache-Control: no-cache\r\n"
)

TYPES = {".wasm": "application/wasm", ".js": "text/javascript", ".json": "application/json"}


async def pipe(reader, writer):
    try:
        while data := await reader.read(65536):
            writer.write(data)
            await writer.drain()
    except ConnectionError:
        pass
    finally:
        writer.close()


def respond(writer, status, body, content_type="text/plain", head_only=False):
    writer.write(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n".format(
            status, content_type, len(body)
        ).encode()
        + HEADERS
        + b"\r\n"
        + (b"" if head_only else body)
    )


def handler(root, backend_host, backend_port):
    async def handle(reader, writer):
        try:
            head = await reader.readuntil(b"\r\n\r\n")
        except (asyncio.IncompleteReadError, asyncio.LimitOverrunError, ConnectionError):
            writer.close()
            return
        method, target, _ = head.split(b"\r\n", 1)[0].decode("latin-1").split(" ", 2)
        path = unquote(urlsplit(target).path)
        if path == "/api" or path.startswith("/api/") or path.startswith("/api?"):
            try:
                backend_reader, backend_writer = await asyncio.open_connection(backend_host, backend_port)
            except OSError as error:
                respond(writer, "502 Bad Gateway", str(error).encode())
                writer.close()
                return
            backend_writer.write(head)
            await asyncio.gather(pipe(reader, backend_writer), pipe(backend_reader, writer))
            return
        if method not in ("GET", "HEAD"):
            respond(writer, "405 Method Not Allowed", b"")
        else:
            file = os.path.normpath(os.path.join(root, path.lstrip("/") or "index.html"))
            if os.path.isdir(file):
                file = os.path.join(file, "index.html")
            if not file.startswith(root) or not os.path.isfile(file):
                respond(writer, "404 Not Found", b"not found")
            else:
                extension = os.path.splitext(file)[1]
                content_type = TYPES.get(extension) or mimetypes.guess_type(file)[0] or "application/octet-stream"
                with open(file, "rb") as opened:
                    body = opened.read()
                respond(writer, "200 OK", body, content_type, head_only=method == "HEAD")
        await writer.drain()
        writer.close()

    return handle


async def main():
    parser = argparse.ArgumentParser(prog="./scripts/buck run //crates/block-app:web-serve --")
    parser.add_argument("--listen", default="127.0.0.1:8080", help="where the page is served")
    parser.add_argument("--backend", default="127.0.0.1:9090", help="where block-server listens")
    parser.add_argument("bundle", help=argparse.SUPPRESS)
    parser.add_argument("server", help=argparse.SUPPRESS)
    arguments, server_arguments = parser.parse_known_args()

    root = os.path.realpath(arguments.bundle)
    host, port = arguments.listen.rsplit(":", 1)
    backend_host, backend_port = arguments.backend.rsplit(":", 1)
    server = subprocess.Popen(
        [arguments.server, "--addr", arguments.backend] + (server_arguments or ["--disable-registration"])
    )
    try:
        listener = await asyncio.start_server(handler(root, backend_host, int(backend_port)), host, int(port))
        print("Serving {} on http://{}".format(root, arguments.listen), flush=True)
        async with listener:
            await listener.serve_forever()
    finally:
        server.terminate()
        server.wait()


try:
    asyncio.run(main())
except KeyboardInterrupt:
    sys.exit(0)
