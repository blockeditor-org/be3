#!/usr/bin/env python3
#
# The web bundle and the server behind it, on this machine:
#
#   ./scripts/buck run //crates/block-app:web-serve
#
# Starts block-server on --backend and Caddy on --listen, with the Caddyfile
# beside this, which a deployment runs as well: the bundle served with the two
# headers that make the page cross-origin isolated, which the module's shared
# memory needs before a browser will hand it out, and /api passed through to
# the server. Here it is plain http on this machine's address rather than TLS
# on a domain.
#
# Anything after the options is handed to block-server; the default is
# --disable-registration, as a deployment runs it.

import argparse
import os
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(prog="./scripts/buck run //crates/block-app:web-serve --")
    parser.add_argument("--listen", default="127.0.0.1:8080", help="where the page is served")
    parser.add_argument("--backend", default="127.0.0.1:9090", help="where block-server listens")
    parser.add_argument("--caddy", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--caddyfile", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--bundle", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--server", required=True, help=argparse.SUPPRESS)
    arguments, server_arguments = parser.parse_known_args()

    caddy = os.path.join(arguments.caddy, "caddy.exe" if os.name == "nt" else "caddy")
    environment = dict(
        os.environ,
        BE3_DOMAIN_NAME="http://" + arguments.listen,
        BE3_BACKEND_URL=arguments.backend,
        BE3_WEB_ROOT=os.path.realpath(arguments.bundle),
    )
    server = subprocess.Popen(
        [arguments.server, "--addr", arguments.backend] + (server_arguments or ["--disable-registration"])
    )
    try:
        print("Serving http://{}".format(arguments.listen), flush=True)
        return subprocess.run(
            [caddy, "run", "--adapter", "caddyfile", "--config", arguments.caddyfile], env=environment
        ).returncode
    except KeyboardInterrupt:
        return 0
    finally:
        server.terminate()
        server.wait()


sys.exit(main())
