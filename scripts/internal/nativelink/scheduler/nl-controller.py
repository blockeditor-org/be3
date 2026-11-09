#!/usr/bin/env python3
"""nl-controller: keeps the build server's worker VMs alive while builds run. Runs on the scheduler VM as root.

What it does, every tick:
  * A build is running when some client holds a connection to the scheduler's frontend port from a client address.
  * While a build runs it keeps WORKERS worker VMs alive: it extends their lifetime before it runs out, and creates
    replacements for any that are gone.
  * A new worker is created with its WireGuard config and bootstrap script handed over at creation (no SSH, which its
    key does not have). Boat places a new VM at random, and about half of them cannot reach the scheduler, so the
    controller creates twice as many as it needs, keeps the first ones that complete a WireGuard handshake with the
    scheduler, and deletes the rest.
  * With no build running it creates nothing: workers run out their own 30-minute lifetime and stop.
  * After SCHEDULER_IDLE_STOP seconds with no build and no worker it stops the scheduler itself.

Its only boat credential is a key limited to the nl-worker environment that can create, read, extend and delete the
workers it created (boat api-key, actions sandbox.create, sandbox.read, sandbox.update, sandbox.delete-own).

  nl-controller.py [--once] [--dry-run] [--force-active]
"""
import argparse
import concurrent.futures as cf
import json
import os
import re
import subprocess
import sys
import tempfile
import time

WORKERS = int(os.environ.get("NL_WORKERS", "4"))
WORKER_TYPE = os.environ.get("NL_WORKER_TYPE", "default")
WORKER_TTL = int(os.environ.get("NL_WORKER_TTL", "1800"))              # a worker's lifetime, extended while builds run
EXTEND_WHEN_LEFT = int(os.environ.get("NL_EXTEND_WHEN_LEFT", "900"))   # extend when less than this is left
SCHEDULER_IDLE_STOP = int(os.environ.get("NL_SCHEDULER_IDLE_STOP", str(4 * 3600)))
TICK = int(os.environ.get("NL_TICK", "15"))
HANDSHAKE_WAIT = int(os.environ.get("NL_HANDSHAKE_WAIT", "60"))        # a good worker handshakes within ~25 s of creation
MAX_CREATES_PER_HOUR = int(os.environ.get("NL_MAX_CREATES_PER_HOUR", "24"))   # a runaway-loop guard, not a budget
FRONTEND_PORT = os.environ.get("NL_FRONTEND_PORT", "50052")
CLIENT_RANGE = (20, 49)     # 10.100.0.20-49: a client that joined the WireGuard network itself
WORKER_RANGE = (51, 99)     # 10.100.0.51-99: workers
NET = "10.100.0."
WG_IF = "wg-sch"
WG_CONF = f"/etc/wireguard/{WG_IF}.conf"
FILES = os.environ.get("NL_FILES", "/opt/nl/worker")
SK_HOME = os.environ.get("NL_BOAT_HOME", "/var/lib/nl-controller/sk-home")
STATE = os.environ.get("NL_STATE", "/var/lib/nl-controller/state.json")

DRY = False


def log(msg):
    print(time.strftime("%H:%M:%S"), msg, flush=True)


def run(cmd, env=None, timeout=300, stdin=None):
    e = dict(os.environ, BOAT_NO_UPDATE="1")
    if env:
        e.update(env)
    p = subprocess.run(cmd, capture_output=True, text=True, env=e, timeout=timeout, input=stdin)
    return p.returncode, p.stdout, p.stderr


def boat(*args, timeout=300):
    return run(["boat", *args], env={"HOME": SK_HOME}, timeout=timeout)


def json_lines(text):
    for line in text.splitlines():
        try:
            yield json.loads(line)
        except ValueError:
            continue


def hub_ipv6():
    _, out, _ = run(["ip", "-6", "-o", "addr", "show", "scope", "global"])
    for line in out.splitlines():
        addr = line.split()[3].split("/")[0]
        if not addr.startswith("fd"):
            return addr
    raise RuntimeError("no global IPv6 address")


def build_running():
    """Is a client connected to the frontend port? A client reaches it through nl-tunnel.py, whose connections come
    from this VM itself; the workers connect too, but from their own address range."""
    _, out, _ = run(["ss", "-Htn", "state", "established", f"( sport = :{FRONTEND_PORT} )"])
    for line in out.splitlines():
        cols = line.split()
        if len(cols) < 4:
            continue
        peer = cols[3].rsplit(":", 1)[0]
        if peer in (f"{NET}1", "127.0.0.1", "::1", "[::1]"):
            return True
        if peer.startswith(NET) and CLIENT_RANGE[0] <= int(peer.rsplit(".", 1)[1]) <= CLIENT_RANGE[1]:
            return True
    return False


REGISTRY = "/run/nl-peers"      # "name pubkey ip" per line, written by add-peer.sh; empty after a restore, like the workers


def registry():
    try:
        return {f[0]: (f[1], f[2]) for f in (l.split() for l in open(REGISTRY) if l.strip())}
    except OSError:
        return {}


def conf_peers():
    """Static peers persisted in the config file (the clients): their keys and addresses."""
    conf = open(WG_CONF).read()
    return set(re.findall(r"PublicKey = (\S+)", conf)), set(re.findall(r"AllowedIPs = ([\d.]+)/32", conf))


def wg_peers():
    """name -> (pubkey, ip) for the worker peers, and pubkey -> seconds since the last handshake."""
    peers = registry()
    _, out, _ = run(["wg", "show", WG_IF, "latest-handshakes"])
    now = time.time()
    age = {}
    for line in out.splitlines():
        k, t = line.split()
        age[k] = now - int(t) if int(t) else None
    return peers, age


def list_workers():
    rc, out, err = boat("--json", "list")
    if rc != 0:
        raise RuntimeError(f"boat list failed: {err.strip() or out.strip()}")
    sandboxes = json.loads(out)["sandboxes"]
    return [s for s in sandboxes if s["name"].startswith("nl-w-")]


def parse_time(ts):
    return time.mktime(time.strptime(ts.split(".")[0], "%Y-%m-%dT%H:%M:%S")) - time.timezone if ts else None


def load_state():
    try:
        return json.load(open(STATE))
    except (OSError, ValueError):
        return {"creates": []}


def save_state(st):
    os.makedirs(os.path.dirname(STATE), exist_ok=True)
    json.dump(st, open(STATE, "w"))


def free_ips(peers):
    used = {ip for _, ip in peers.values()} | conf_peers()[1]
    return [f"{NET}{n}" for n in range(WORKER_RANGE[0], WORKER_RANGE[1] + 1) if f"{NET}{n}" not in used]


def create_candidate(name, ip, hub, tmp):
    """Create one worker VM. Returns its sandbox id, or None."""
    rc, conf, err = run(["/usr/local/bin/add-peer.sh", name, ip, hub])
    if rc != 0:
        log(f"  add-peer {name} failed: {err.strip()}")
        return None
    wg = os.path.join(tmp, f"{name}.wg")
    with open(os.open(wg, os.O_WRONLY | os.O_CREAT, 0o600), "w") as f:
        f.write(conf)
    cfg = os.path.join(tmp, f"{name}.json5")
    with open(os.path.join(FILES, "worker-direct.json5")) as f:
        text = f.read().replace("10.99.0.1", "10.100.0.1")
    with open(cfg, "w") as f:
        f.write(text)
    rc, out, err = boat("new", "--name", name, "--environment", "nl-worker", "--type", WORKER_TYPE,
                        "--ttl", str(WORKER_TTL), "--no-snapshots",
                        "--setup-file", os.path.join(FILES, "worker-bootstrap.sh"),
                        "--file", f"/home/user/nl-wg.conf={wg}",
                        "--file", f"/home/user/nl-worker.json5={cfg}",
                        "--file", f"/home/user/nl-worker.service={os.path.join(FILES, 'nativelink-worker-sch.service')}")
    # The CLI exits with an error after the VM is up (it tries an exec this key may not run), so read the id from the
    # events and do not trust the exit code.
    for ev in json_lines(out):
        if ev.get("event") == "created" and ev.get("id"):
            return ev["id"]
    log(f"  create {name} failed: {(err or out).strip()[-200:]}")
    return None


def spawn(missing):
    """Create 2x the workers needed, keep the first `missing` that handshake with the scheduler, delete the rest."""
    st = load_state()
    now = time.time()
    st["creates"] = [t for t in st["creates"] if now - t < 3600]
    make = missing * 2
    if len(st["creates"]) + make > MAX_CREATES_PER_HOUR:
        log(f"  refusing to create {make}: {len(st['creates'])} created in the last hour (limit {MAX_CREATES_PER_HOUR})")
        return
    peers, _ = wg_peers()
    ips = free_ips(peers)
    if len(ips) < make:
        log("  not enough free worker addresses")
        return
    hub = hub_ipv6()
    stamp = int(now)
    names = [f"nl-w-{stamp}-{i}" for i in range(make)]
    if DRY:
        log(f"  [dry-run] would create {make} candidates {names[0]}.. for {missing} workers")
        return
    st["creates"] += [now] * make
    save_state(st)
    with tempfile.TemporaryDirectory() as tmp, cf.ThreadPoolExecutor(max_workers=make) as pool:
        ids = list(pool.map(lambda a: create_candidate(a[0], a[1], hub, tmp), zip(names, ips)))
    created = {n: i for n, i in zip(names, ids) if i}
    log(f"  created {len(created)}/{make}; waiting up to {HANDSHAKE_WAIT}s for handshakes")
    kept = []
    deadline = time.time() + HANDSHAKE_WAIT
    while time.time() < deadline and len(kept) < missing:
        peers, age = wg_peers()
        for n in created:
            if n not in kept and n in peers and age.get(peers[n][0]) is not None and age[peers[n][0]] < 120:
                kept.append(n)
        if len(kept) < missing:
            time.sleep(3)
    kept = kept[:missing]
    for n, i in created.items():
        if n not in kept:
            boat("delete", "--yes", i)
            run(["/usr/local/bin/remove-peer.sh", n])
    log(f"  kept {len(kept)}/{missing}: {kept}; deleted {len(created) - len(kept)} that did not reach the scheduler")


def reap_and_extend(workers, active):
    names = {w["name"] for w in workers}
    peers, _ = wg_peers()
    # The running interface must match the config file: drop any peer that is no longer in the file.
    _, live, _ = run(["wg", "show", WG_IF, "peers"])
    known = {pub for pub, _ in peers.values()} | conf_peers()[0]
    for key in live.split():
        if key not in known and not DRY:
            log(f"  removing a peer that is in neither the registry nor {WG_CONF}")
            run(["wg", "set", WG_IF, "peer", key, "remove"])
    for n in peers:
        if n.startswith("nl-w-") and n not in names and not DRY:
            log(f"  removing the peer of the vanished worker {n}")
            run(["/usr/local/bin/remove-peer.sh", n])
    if not active:
        return
    for w in workers:
        left = (parse_time(w.get("archiveAfter")) or 0) - time.time()
        if left < EXTEND_WHEN_LEFT:
            log(f"  extending {w['name']} ({int(left)}s left)")
            if not DRY:
                boat("extend", w["id"], "--ttl", str(WORKER_TTL))


def tick(state, force_active):
    active = force_active or build_running()
    if active:
        state["last_active"] = time.time()
    workers = [w for w in list_workers() if w["state"] not in ("stopped", "archived", "deleting")]
    peers, age = wg_peers()
    alive = [w for w in workers if w["name"] in peers and age.get(peers[w["name"]][0]) is not None
             and age[peers[w["name"]][0]] < 180]
    starting = [w for w in workers if w not in alive and time.time() - (parse_time(w.get("createdAt")) or 0) < 120]
    log(f"build running: {active} | workers: {len(workers)} ({len(alive)} connected, {len(starting)} starting)")
    reap_and_extend(workers, active)
    if active:
        missing = WORKERS - len(alive) - len(starting)
        if missing > 0:
            log(f"  need {missing} more workers")
            spawn(missing)
    elif not workers and time.time() - state["last_active"] > SCHEDULER_IDLE_STOP:
        log("idle past the limit with no workers: stopping the scheduler")
        if not DRY:
            run(["boat", "stop", "self"], env={"HOME": "/home/user"})


def main():
    global DRY
    ap = argparse.ArgumentParser()
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--dry-run", action="store_true", help="decide and log, but create, delete, extend and stop nothing")
    ap.add_argument("--force-active", action="store_true", help="act as if a build were running (testing)")
    a = ap.parse_args()
    DRY = a.dry_run
    state = {"last_active": time.time()}    # a freshly woken scheduler gets the full idle time
    log(f"nl-controller: {WORKERS} x {WORKER_TYPE} workers, worker ttl {WORKER_TTL}s, scheduler idle stop {SCHEDULER_IDLE_STOP}s"
        + (" [dry-run]" if DRY else ""))
    while True:
        try:
            tick(state, a.force_active)
        except Exception as e:      # keep going: a boat API hiccup must not end the controller
            log(f"tick failed: {type(e).__name__}: {e}")
        if a.once:
            return
        time.sleep(TICK)


if __name__ == "__main__":
    sys.exit(main())
