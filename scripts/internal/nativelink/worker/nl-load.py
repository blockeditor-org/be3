#!/usr/bin/env python3
"""nl-load: tells the controller how busy this worker is. GET /load answers {"busy": percent, "load1": n, "cpus": n}.

Listens on this worker's WireGuard address only, so nothing but the scheduler (and the other workers) can ask. The
controller scales the worker fleet on the average of these numbers: NativeLink exposes no queue depth, and "are my
workers saturated" is the question scaling needs answered anyway. Standard library only.
"""
import json
import os
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(os.environ.get("NL_LOAD_PORT", "9100"))
WINDOW = 30       # seconds the busy figure averages over
samples = []      # (time, busy jiffies, total jiffies)


def cpu_times():
    fields = [int(x) for x in open("/proc/stat").readline().split()[1:]]
    user, nice, system, idle, iowait, irq, softirq = fields[:7]
    return user + nice + system + irq + softirq, sum(fields[:8])


def sampler():
    while True:
        busy, total = cpu_times()
        samples.append((time.time(), busy, total))
        del samples[:-12]
        time.sleep(5)


def busy_percent():
    if len(samples) < 2:
        return 0.0
    now = time.time()
    old = next((s for s in samples if now - s[0] <= WINDOW), samples[0])
    new = samples[-1]
    return 100.0 * (new[1] - old[1]) / max(1, new[2] - old[2])


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/load":
            self.send_error(404)
            return
        body = json.dumps({"busy": round(busy_percent(), 1), "load1": float(open("/proc/loadavg").read().split()[0]),
                           "cpus": os.cpu_count()}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def wireguard_address():
    for _ in range(60):
        out = subprocess.run(["ip", "-4", "-o", "addr", "show", "dev", "wg-sch"], capture_output=True, text=True).stdout
        if out.strip():
            return out.split()[3].split("/")[0]
        time.sleep(2)
    raise SystemExit("nl-load: no wg-sch address")


if __name__ == "__main__":
    threading.Thread(target=sampler, daemon=True).start()
    ThreadingHTTPServer((wireguard_address(), PORT), Handler).serve_forever()
