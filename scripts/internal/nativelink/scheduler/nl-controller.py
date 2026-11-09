#!/usr/bin/env python3
"""nl-controller: keeps the build server's worker VMs alive while builds run. Runs on the scheduler VM as root.

What it does, every tick:
  * A build is running when some client holds a connection to the scheduler's frontend port from a client address.
  * While a build runs it keeps BASE worker VMs alive and scales between BASE and CAP on the queue: NativeLink reports
    how many actions are queued (waiting for a worker) and executing, each worker takes SLOTS at once, and work that
    has waited means more workers are needed. It extends their lifetime before it runs out and creates replacements for
    any that are gone. Workers beyond BASE are a burst: they live a short time and are simply not extended once the
    queue has been short for a while.
  * A new worker is created with its WireGuard config and bootstrap script handed over at creation (no SSH, which its
    key does not have). Boat places a new VM at random, and about half of them cannot reach the scheduler, so the
    controller creates enough to expect the ones it needs (it learns the share), keeps the ones that complete a
    WireGuard handshake with the scheduler, and deletes the rest. Every VM created is a start, which boat limits and
    charges for past the plan's included number, so it works within an allowance (see start_allowance).
  * With no build running it creates nothing: workers run out their own 30-minute lifetime and stop.
  * After SCHEDULER_IDLE_STOP seconds with no build and no worker it stops the scheduler itself.

Its only boat credential is a key limited to the nl-worker environment that can create, read, extend and delete the
workers it created (boat api-key, actions sandbox.create, sandbox.read, sandbox.update, sandbox.delete-own).

  nl-controller.py [--once] [--dry-run] [--force-active]
"""
import argparse
import concurrent.futures as cf
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.request

BASE = int(os.environ.get("NL_WORKERS_BASE", "4"))           # workers kept while any build runs
CAP = int(os.environ.get("NL_WORKERS_CAP", "12"))            # the most it will burst to
UP_AFTER = int(os.environ.get("NL_UP_AFTER", "20"))          # seconds work must have waited before adding workers
UP_EVERY = int(os.environ.get("NL_UP_EVERY", "45"))          # seconds between adding workers
DOWN_AFTER = int(os.environ.get("NL_DOWN_AFTER", "300"))     # seconds with spare workers before they are let go
BURST_TTL = int(os.environ.get("NL_BURST_TTL", "600"))       # a burst worker lives this long, and is extended while wanted
WORKER_TYPE = os.environ.get("NL_WORKER_TYPE", "default")
WORKER_TTL = int(os.environ.get("NL_WORKER_TTL", "1800"))    # a worker's lifetime, extended while builds run
EXTEND_WHEN_LEFT = int(os.environ.get("NL_EXTEND_WHEN_LEFT", "900"))   # extend when less than this is left
SCHEDULER_IDLE_STOP = int(os.environ.get("NL_SCHEDULER_IDLE_STOP", str(4 * 3600)))
TICK = int(os.environ.get("NL_TICK", "15"))
HANDSHAKE_WAIT = int(os.environ.get("NL_HANDSHAKE_WAIT", "60"))        # a good worker handshakes within ~25 s of creation
METRICS_URL = os.environ.get("NL_METRICS_URL", "http://127.0.0.1:8889/metrics")   # nl-otelcol: NativeLink's queue depth

# Starts. Every VM created is a start, and boat counts them per minute, hour and day against what the plan includes.
# A start over that costs $0.03 (up to 1.5x the included number), $0.05 (up to 2x) or $0.07 (up to 6x, 3x a minute),
# whichever window is highest, and fails above it (docs.boat.dev/limits#extra-starts). Over-provisioning spends starts
# on VMs that are deleted at once, so past the included number it costs money. By default the controller never pays
# for a start: it leaves RESERVE of each window for everybody else's VMs and stops when the rest is used.
INCLUDED = {"day": 200, "hour": 60, "minute": 12}            # the $20 plan, used when the key cannot read `boat limits`
RESERVE = {"day": int(os.environ.get("NL_RESERVE_DAY", "40")), "hour": int(os.environ.get("NL_RESERVE_HOUR", "10")),
           "minute": 0}
MAX_START_PRICE = float(os.environ.get("NL_MAX_START_PRICE", "0"))       # dollars: the dearest start it will pay for
EXTRA_START_BUDGET = float(os.environ.get("NL_EXTRA_START_BUDGET", "0"))  # dollars a month it may spend on extra starts
FRONTEND_PORT = os.environ.get("NL_FRONTEND_PORT", "50052")
CLIENT_RANGE = (20, 49)     # 10.100.0.20-49: a client that joined the WireGuard network itself
WORKER_RANGE = (51, 99)     # 10.100.0.51-99: workers
NET = "10.100.0."
WG_IF = "wg-sch"
WG_CONF = f"/etc/wireguard/{WG_IF}.conf"
FILES = os.environ.get("NL_FILES", "/opt/nl/worker")


def worker_slots():
    """How many actions a worker runs at once: max_inflight_tasks in its config, which is what makes work queue."""
    try:
        return max(1, int(re.search(r"max_inflight_tasks:\s*(\d+)", open(os.path.join(FILES, "worker-direct.json5")).read()).group(1)))
    except (OSError, AttributeError):
        return 8


SLOTS = worker_slots()
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
        return {"creates": [], "p": 0.5, "extra_spend": []}


def save_state(st):
    os.makedirs(os.path.dirname(STATE), exist_ok=True)
    json.dump(st, open(STATE, "w"))


def free_ips(peers):
    used = {ip for _, ip in peers.values()} | conf_peers()[1]
    return [f"{NET}{n}" for n in range(WORKER_RANGE[0], WORKER_RANGE[1] + 1) if f"{NET}{n}" not in used]


def create_candidate(name, ip, hub, tmp, ttl):
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
                        "--ttl", str(ttl), "--no-snapshots",
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


def limits():
    """Starts left in each window and the price of the next one, if this key may read `boat limits` (account.read)."""
    rc, out, _ = boat("--json", "limits", timeout=30)
    if rc != 0:
        return None
    try:
        d = json.loads(out)
        left = {w: d["starts"][w]["remaining"] for w in ("day", "hour", "minute")}
        return {**left, "price": float(d.get("nextStartDollars") or 0)}
    except (ValueError, KeyError, TypeError):
        return None


def start_allowance(want, st):
    """How many VMs may be created now, and what each start over the included number costs. Free starts come first,
    minus the reserve; paid ones only when a price and a monthly budget allow."""
    now = time.time()
    lim = limits()
    if lim is None:
        spans = {"day": 86400, "hour": 3600, "minute": 60}
        left = {w: INCLUDED[w] - sum(1 for t in st["creates"] if now - t < spans[w]) for w in spans}
        price = 0.0
    else:
        left, price = {w: lim[w] for w in ("day", "hour", "minute")}, lim["price"]
    free = max(0, min(left[w] - RESERVE[w] for w in left))
    allowed = min(want, free)
    extra = 0
    if lim is not None and allowed < want and price > 0 and MAX_START_PRICE >= price:
        spent = sum(d for t, d in st.get("extra_spend", []) if now - t < 30 * 86400)
        extra = min(want - allowed, int(max(0.0, EXTRA_START_BUDGET - spent) // price))
    return allowed + extra, price, extra


def demand():
    """(queued, executing) actions, from NativeLink's execution.active.count as the collector serves it, or None."""
    try:
        with urllib.request.urlopen(METRICS_URL, timeout=3) as response:
            text = response.read().decode()
    except OSError:
        return None
    totals = {}
    for line in text.splitlines():
        m = re.match(r"nativelink_execution_active_count\{([^}]*)\}\s+([0-9.eE+-]+)", line)
        stage = re.search(r'execution_stage="([^"]*)"', m.group(1)) if m else None
        if stage:
            totals[stage.group(1)] = totals.get(stage.group(1), 0.0) + float(m.group(2))
    return max(0, round(totals.get("queued", 0))), max(0, round(totals.get("executing", 0)))


def spawn(missing, burst=False):
    """Create enough VMs to expect `missing` that reach the scheduler (a share p of them do, which it learns), keep
    the ones that handshake, delete the rest, within the start allowance."""
    st = load_state()
    now = time.time()
    st["creates"] = [t for t in st["creates"] if now - t < 86400]
    st["extra_spend"] = [(t, d) for t, d in st.get("extra_spend", []) if now - t < 30 * 86400]
    if st.get("backoff_until", 0) > now:
        log(f"  boat refused a start recently; waiting {int(st['backoff_until'] - now)}s")
        return
    p = min(0.9, max(0.35, st.get("p", 0.5)))
    want = min(3 * missing, max(missing, math.ceil(missing / p)))
    make, price, extra = start_allowance(want, st)
    if make < want:
        log(f"  start budget: wanted {want} starts (reachable share {p:.2f}), may use {make}"
            + (f", {extra} of them at ${price:.2f}" if extra else ""))
    if make == 0:
        return
    peers, _ = wg_peers()
    ips = free_ips(peers)
    if len(ips) < make:
        log("  not enough free worker addresses")
        return
    hub = hub_ipv6()
    stamp = int(now)
    names = [f"nl-w-{'b' if burst else ''}{stamp}-{i}" for i in range(make)]
    ttl = BURST_TTL if burst else WORKER_TTL
    if DRY:
        log(f"  [dry-run] would create {make} candidates {names[0]}.. for {missing} workers")
        return
    with tempfile.TemporaryDirectory() as tmp, cf.ThreadPoolExecutor(max_workers=make) as pool:
        ids = list(pool.map(lambda a: create_candidate(a[0], a[1], hub, tmp, ttl), zip(names, ips)))
    created = {n: i for n, i in zip(names, ids) if i}
    st["creates"] += [now] * len(created)
    if extra:
        st["extra_spend"] += [(now, price)] * min(extra, len(created))
    if len(created) < make:
        st["backoff_until"] = now + 600        # boat refused some: its limits are reached
    log(f"  created {len(created)}/{make}; waiting up to {HANDSHAKE_WAIT}s for handshakes")
    good = []
    deadline = time.time() + HANDSHAKE_WAIT
    while time.time() < deadline and len(good) < len(created):
        peers, age = wg_peers()
        for n in created:
            if n not in good and n in peers and age.get(peers[n][0]) is not None and age[peers[n][0]] < 120:
                good.append(n)
        if len(good) >= missing + 1:
            break
        time.sleep(3)
    if created:
        time.sleep(4)       # the stragglers of this round: they reach the scheduler within seconds or not at all
        peers, age = wg_peers()
        good = [n for n in created if n in peers and age.get(peers[n][0]) is not None and age[peers[n][0]] < 120]
        st["p"] = 0.7 * p + 0.3 * (len(good) / len(created))
    kept = good[: missing + 1]       # a spare that already cost a start is kept: the fleet may grow
    for n, i in created.items():
        if n not in kept:
            boat("delete", "--yes", i)
            run(["/usr/local/bin/remove-peer.sh", n])
    save_state(st)
    log(f"  kept {len(kept)} for {missing} needed; deleted {len(created) - len(kept)}; reachable share now {st['p']:.2f}")


def reap_and_extend(workers, active, wanted):
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
        if w["name"] not in wanted:
            continue
        burst = w["name"].startswith("nl-w-b")
        left = (parse_time(w.get("archiveAfter")) or 0) - time.time()
        if left < (BURST_TTL // 2 if burst else EXTEND_WHEN_LEFT):
            log(f"  extending {w['name']} ({int(left)}s left)")
            if not DRY:
                boat("extend", w["id"], "--ttl", str(BURST_TTL if burst else WORKER_TTL))


def tick(state, force_active):
    now = time.time()
    active = force_active or build_running()
    if active:
        state["last_active"] = now
    workers = [w for w in list_workers() if w["state"] not in ("stopped", "archived", "deleting")]
    peers, age = wg_peers()
    alive = [w for w in workers if w["name"] in peers and age.get(peers[w["name"]][0]) is not None
             and age[peers[w["name"]][0]] < 180]
    starting = [w for w in workers if w not in alive and now - (parse_time(w.get("createdAt")) or 0) < 120]
    queue = demand() if active else None
    target = max(BASE, state.get("target", BASE)) if active else BASE
    note = ""
    if active and queue is not None:
        queued, executing = queue
        needed = math.ceil((queued + executing) / SLOTS)
        note = f" | queued {queued}, executing {executing} (needs {needed} at {SLOTS} each)"
        # Work that has waited: add workers, as many as the demand needs, at most every UP_EVERY seconds.
        state["queued_since"] = (state.get("queued_since") or now) if queued > 0 else None
        if (queued > 0 and now - state["queued_since"] >= UP_AFTER and now - state.get("last_up", 0) >= UP_EVERY
                and target < CAP and not starting):
            target = min(CAP, max(target + 1, needed))
            state["last_up"] = now
            log(f"  {queued} actions have waited {int(now - state['queued_since'])}s: scaling up to {target}")
        # Spare workers: not extended once there has been no queue and room to spare for DOWN_AFTER.
        if queued == 0 and needed < target and target > BASE:
            state["slack_since"] = state.get("slack_since") or now
            if now - state["slack_since"] >= DOWN_AFTER:
                target = max(BASE, needed)
                state["slack_since"] = None
                log(f"  no queue for {DOWN_AFTER}s: scaling down to {target}")
        else:
            state["slack_since"] = None
    elif active:
        note = " | no queue metrics (nl-otelcol), holding at the base"
        target = BASE
    state["target"] = target
    log(f"build running: {active} | workers: {len(workers)} ({len(alive)} connected, {len(starting)} starting) | target {target}{note}")
    wanted = {w["name"] for w in sorted(alive + starting, key=lambda w: parse_time(w.get("createdAt")) or 0)[:target]}
    reap_and_extend(workers, active, wanted)
    if active:
        missing = target - len(alive) - len(starting)
        if missing > 0:
            log(f"  need {missing} more workers")
            spawn(missing, burst=len(alive) + len(starting) >= BASE)
    elif not workers and now - state["last_active"] > SCHEDULER_IDLE_STOP:
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
    log(f"nl-controller: {BASE}-{CAP} x {WORKER_TYPE} workers, worker ttl {WORKER_TTL}s, scheduler idle stop {SCHEDULER_IDLE_STOP}s, "
        f"start price limit ${MAX_START_PRICE:.2f}, extra-start budget ${EXTRA_START_BUDGET:.2f}/month"
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
