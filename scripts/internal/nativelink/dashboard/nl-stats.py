#!/usr/bin/env python3
"""The build server's records of builds, and its count of running actions (nl-stats.service).

- POST /__be3-stats/build (Caddy checks the token and forwards it here) takes the JSON record
  crates/be-lib-build-record makes of each build (guides/build_server.md, The dashboard), and stores it in
  /var/lib/nl-stats/builds.db, which Grafana reads (grafana-datasources.yaml).
- Every 5 s it counts the actions running on the worker, one directory each under /var/lib/nativelink/run, and
  writes the count where node_exporter's textfile collector reads it, as nl_actions_running.

Python's standard library only, so nothing is installed for it.
"""

import json
import os
import sqlite3
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LISTEN = ("127.0.0.1", 50080)
DATABASE = "/var/lib/nl-stats/builds.db"
RUN = "/var/lib/nativelink/run"
TEXTFILE = "/var/lib/prometheus/node-exporter/nl.prom"
MAX_BODY = 8 << 20

SCHEMA = """
CREATE TABLE IF NOT EXISTS builds (
    id TEXT PRIMARY KEY,
    started REAL NOT NULL,
    seconds REAL NOT NULL,
    kind TEXT NOT NULL,
    commit_sha TEXT,
    branch TEXT,
    parts TEXT,
    passed INTEGER NOT NULL,
    actions INTEGER,
    cached INTEGER,
    remote INTEGER,
    local INTEGER,
    remote_seconds REAL,
    queue_seconds REAL
);
CREATE INDEX IF NOT EXISTS builds_started ON builds (started);
-- Each action of a build that ran and took long, and every test run: what the slowest tests and
-- compiles are, and how they change.
CREATE TABLE IF NOT EXISTS actions (
    build_id TEXT NOT NULL REFERENCES builds (id) ON DELETE CASCADE,
    started REAL NOT NULL,
    target TEXT NOT NULL,
    category TEXT NOT NULL,
    seconds REAL NOT NULL,
    queue_seconds REAL,
    failed INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS actions_started ON actions (started);
CREATE INDEX IF NOT EXISTS actions_build ON actions (build_id);
-- The build's critical path, in order.
CREATE TABLE IF NOT EXISTS critical_path (
    build_id TEXT NOT NULL REFERENCES builds (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    what TEXT NOT NULL,
    seconds REAL NOT NULL,
    queue_seconds REAL
);
CREATE INDEX IF NOT EXISTS critical_path_build ON critical_path (build_id);
"""


def connect():
    database = sqlite3.connect(DATABASE, timeout=30)
    database.execute("PRAGMA foreign_keys = ON")
    return database


def store(record):
    build = record["id"]
    started = float(record["started"])
    actions = record.get("actions", {})
    with connect() as database:
        database.execute("DELETE FROM builds WHERE id = ?", (build,))
        database.execute(
            "INSERT INTO builds VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                build,
                started,
                float(record["seconds"]),
                str(record["kind"]),
                record.get("commit"),
                record.get("branch"),
                record.get("parts"),
                1 if record.get("passed") else 0,
                actions.get("total"),
                actions.get("cached"),
                actions.get("remote"),
                actions.get("local"),
                record.get("remote_seconds"),
                record.get("queue_seconds"),
            ),
        )
        database.executemany(
            "INSERT INTO actions VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                (
                    build,
                    started,
                    str(action["target"]),
                    str(action["category"]),
                    float(action["seconds"]),
                    action.get("queue_seconds"),
                    1 if action.get("failed") else 0,
                )
                for action in record.get("slow", [])
            ],
        )
        database.executemany(
            "INSERT INTO critical_path VALUES (?, ?, ?, ?, ?)",
            [
                (build, position, str(step["what"]), float(step["seconds"]), step.get("queue_seconds"))
                for position, step in enumerate(record.get("critical_path", []))
            ],
        )


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/__be3-stats/build":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length") or 0)
        if not 0 < length <= MAX_BODY:
            self.send_error(413 if length else 411)
            return
        try:
            store(json.loads(self.rfile.read(length)))
        except (ValueError, KeyError, TypeError) as error:
            self.send_error(400, str(error))
            return
        self.send_response(204)
        self.end_headers()

    def log_message(self, format, *args):
        pass


def count_running():
    while True:
        try:
            running = sum(1 for entry in os.scandir(RUN) if entry.is_dir())
            partial = TEXTFILE + ".tmp"
            with open(partial, "w") as file:
                file.write("# HELP nl_actions_running Actions running on the worker, of its 16 slots.\n")
                file.write("# TYPE nl_actions_running gauge\n")
                file.write(f"nl_actions_running {running}\n")
            os.replace(partial, TEXTFILE)
        except OSError:
            pass
        time.sleep(5)


def main():
    os.makedirs(os.path.dirname(DATABASE), exist_ok=True)
    with connect() as database:
        database.executescript(SCHEMA)
    os.chmod(DATABASE, 0o644)
    threading.Thread(target=count_running, daemon=True).start()
    ThreadingHTTPServer(LISTEN, Handler).serve_forever()


if __name__ == "__main__":
    main()
