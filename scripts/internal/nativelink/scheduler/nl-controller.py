#!/usr/bin/env python3
"""nl-controller: keeps the build server's worker VMs alive while builds run. Runs on the scheduler VM as root.

What it does, every tick:
  * A build is running when some client holds a connection to the scheduler's frontend port from a client address.
  * While a build runs it keeps BASE worker VMs alive and scales between BASE and the cap on the queue: NativeLink
    reports how many actions are queued (waiting for a worker) and executing, each worker takes SLOTS at once, and work
    that has waited means more workers are needed. It extends their lifetime before it runs out and creates
    replacements for any that are gone. Workers beyond BASE are a burst: they live a short time and are simply not
    extended once the queue has been short for a while.
  * The cap follows the plan's VM time (see pace): BASE workers and the scheduler are always allowed, and only what
    the period has left above that floor buys workers past BASE, up to CAP_MAX.
  * A new worker is created with its WireGuard config and bootstrap script handed over at creation (no SSH, which its
    key does not have). Boat places a new VM at random, and about half of them cannot reach the scheduler, so the
    controller creates enough to expect the ones it needs (it learns the share), keeps the ones that complete a
    WireGuard handshake with the scheduler, and deletes the rest. Every VM created is a start, which boat limits and
    charges for past the plan's included number, so it works within an allowance (see start_allowance).
  * With no build running it creates nothing: workers run out their own 30-minute lifetime and stop.
  * After SCHEDULER_IDLE_STOP seconds with no build and no worker it packs the small tier and stops the scheduler itself.

Its only boat credential is a key limited to the nl-worker environment that can create, read, extend and delete the
workers it created (boat api-key, actions sandbox.create, sandbox.read, sandbox.update, sandbox.delete-own), and read
`boat limits` (account.read).

  nl-controller.py [--once] [--dry-run] [--force-active] [--limits FILE]
"""
import argparse
import calendar
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

BASE = int(os.environ.get("NL_WORKERS_BASE", "2"))           # workers kept while any build runs
# The most it will burst to. 12 large workers are 96 CPUs. The large scheduler carried 6 workers' writes at 87% idle,
# so 12 would keep it about 75% idle; the worker addresses allow 49, and a full cold fleet is about 2 starts a worker.
CAP_MAX = int(os.environ.get("NL_WORKERS_CAP_MAX", "12"))
UP_AFTER = int(os.environ.get("NL_UP_AFTER", "60"))          # seconds work must have waited before adding a worker
UP_EVERY = int(os.environ.get("NL_UP_EVERY", "60"))          # seconds at least between adding workers
DOWN_AFTER = int(os.environ.get("NL_DOWN_AFTER", "300"))     # seconds with spare workers before they are let go
BURST_TTL = int(os.environ.get("NL_BURST_TTL", "1800"))      # a burst worker lives this long, and is extended while wanted;
                                                             # 10 minutes let bursts lapse between a build's phases and made
                                                             # them again, a start each time
# large: boat prices every size the same per CPU, so fewer, bigger workers cost the same and use fewer starts (each
# VM created is one, whether or not it reaches the scheduler). max_inflight_tasks in worker-direct.json5 matches it.
WORKER_TYPE = os.environ.get("NL_WORKER_TYPE", "large")
WORKER_TTL = int(os.environ.get("NL_WORKER_TTL", "1800"))    # a worker's lifetime, extended while builds run
EXTEND_WHEN_LEFT = int(os.environ.get("NL_EXTEND_WHEN_LEFT", "900"))   # extend when less than this is left
SCHEDULER_IDLE_STOP = int(os.environ.get("NL_SCHEDULER_IDLE_STOP", str(4 * 3600)))

# Money. The plan includes VM time for each billing period (`boat limits`: planIncluded.vmHoursRemaining, in hours of a
# default VM, and periodEnd); past it, VM time comes out of the wallet. Every sandbox of the account draws on it, this
# scheduler included, so boat's count is the meter. The floor, BASE workers for FLOOR_HOURS of builds a day and the
# scheduler for those hours plus its idle stop, is always allowed; the cap past BASE spends only the rest, spread over
# the hours left in the period (pace).
RATES = {"small": 0.018, "default": 0.036, "large": 0.072, "xlarge": 0.20}   # dollars an hour; xlarge costs more per CPU
WORKER_RATE = RATES[WORKER_TYPE]
SCHEDULER_RATE = RATES[{2: "small", 4: "default", 8: "large", 16: "xlarge"}.get(os.cpu_count(), "large")]
FLOOR_HOURS = float(os.environ.get("NL_FLOOR_HOURS", "12"))   # hours of builds a day the floor pays for
BURST_K = float(os.environ.get("NL_BURST_K", "4"))   # 1 / the share of the time burst workers run, until state measures it
K_RANGE = (1.0, 8.0)                                  # what a measured K is held to
K_HISTORY = 7 * 86400                                 # burst use measured over this long, once it covers a day
BUDGET_EVERY = int(os.environ.get("NL_BUDGET_EVERY", "300"))   # seconds between moves of the cap, one worker each
IDLE_DELETE = int(os.environ.get("NL_IDLE_DELETE", "600"))     # idle this long with nothing to spare: delete bursts early
TICK = int(os.environ.get("NL_TICK", "15"))
HANDSHAKE_WAIT = int(os.environ.get("NL_HANDSHAKE_WAIT", "60"))        # a good worker handshakes within ~25 s of creation
ROUNDS = int(os.environ.get("NL_SPAWN_ROUNDS", "4"))                   # rounds of replacing VMs that cannot reach it
METRICS_URL = os.environ.get("NL_METRICS_URL", "http://127.0.0.1:8889/metrics")   # nl-otelcol: NativeLink's queue depth

# Starts. Every VM created is a start, and boat counts them per minute, hour and day against what the plan includes.
# A start over that costs $0.03 (up to 1.5x the included number), $0.05 (up to 2x) or $0.07 (up to 6x, 3x a minute),
# whichever window is highest, and fails above it (docs.boat.dev/limits#extra-starts). Over-provisioning spends starts
# on VMs that are deleted at once, so past the included number it costs money. By default the controller never pays
# for a start: it leaves RESERVE of each window for everybody else's VMs and stops when the rest is used.
# The smallest plan's, used only before any `boat limits` reading has been saved: never more than any plan includes.
INCLUDED = {"day": 200, "hour": 60, "minute": 12}
RESERVE = {"day": int(os.environ.get("NL_RESERVE_DAY", "5")), "hour": int(os.environ.get("NL_RESERVE_HOUR", "10")),
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
LIMITS_FILE = None          # --limits: read this canned `boat limits --json` instead of asking boat (testing)


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
    """Is a client connected to the frontend port? A client reaches it through Caddy, whose connections come
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
    return calendar.timegm(time.strptime(ts.split(".")[0].rstrip("Z"), "%Y-%m-%dT%H:%M:%S")) if ts else None


def load_state():
    try:
        return json.load(open(STATE))
    except (OSError, ValueError):
        return {"creates": [], "p": 0.5, "extra_spend": []}


def save_state(st):
    if DRY:
        return
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


def parse_limits(text, now):
    """The parts of `boat limits --json` the controller uses, or None."""
    try:
        d = json.loads(text)
        inc = d["planIncluded"]
        return {"at": now,
                "starts": {w: {"limit": int(d["starts"][w]["limit"]), "remaining": int(d["starts"][w]["remaining"])}
                           for w in ("day", "hour", "minute")},
                "price": float(d.get("nextStartDollars") or 0),
                "vm_hours_left": float(inc["vmHoursRemaining"]),
                "vm_hour_dollars": float((d.get("overagePrices") or {}).get("vmHourDollars") or 0.036),
                "period_end": parse_time(inc["periodEnd"]),
                "wallet": float((d.get("wallet") or {}).get("balanceDollars") or 0)}
    except (ValueError, KeyError, TypeError, AttributeError):
        return None


def limits(st):
    """The account's limits, read live if this key may (account.read), and saved in st as the last good reading. When
    boat does not answer, the saved reading, marked stale (its starts left are then counted from st's own creates);
    None only before any reading."""
    now = time.time()
    if LIMITS_FILE:
        text = open(LIMITS_FILE).read()
    else:
        rc, text, _ = boat("--json", "limits", timeout=30)
        text = text if rc == 0 else ""
    lim = parse_limits(text, now)
    if lim is not None:
        st["limits"] = lim
        return dict(lim, stale=False)
    return dict(st["limits"], stale=True) if st.get("limits") else None


def start_allowance(want, st):
    """How many VMs may be created now, and what each start over the included number costs. Free starts come first,
    minus the reserve; paid ones only when a price and a monthly budget allow."""
    now = time.time()
    lim = limits(st)
    if lim is None or lim["stale"]:
        spans = {"day": 86400, "hour": 3600, "minute": 60}
        included = {w: lim["starts"][w]["limit"] for w in spans} if lim else INCLUDED
        left = {w: included[w] - sum(1 for t in st["creates"] if now - t < spans[w]) for w in spans}
        price = 0.0
    else:
        left, price = {w: lim["starts"][w]["remaining"] for w in ("day", "hour", "minute")}, lim["price"]
    free = max(0, min(left[w] - RESERVE[w] for w in left))
    allowed = min(want, free)
    extra = 0
    if lim is not None and not lim["stale"] and allowed < want and price > 0 and MAX_START_PRICE >= price:
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


def floor_rate():
    """Dollars an hour, averaged over the day, that the guaranteed floor costs: BASE workers for FLOOR_HOURS of builds,
    and the scheduler for those hours and the idle stop after them."""
    return (BASE * WORKER_RATE * FLOOR_HOURS + SCHEDULER_RATE * min(24.0, FLOOR_HOURS + SCHEDULER_IDLE_STOP / 3600)) / 24


def pace(lim, now, k):
    """How many workers past BASE the plan's VM time allows now: what the period has left over the floor's share of the
    hours left is a rate in dollars an hour (surplus_rate), and burst workers run about a share 1/k of the time, so
    k * surplus_rate / WORKER_RATE of them may run while they do. Early in a period the floor's whole remaining cost
    is held back; time the floor did not use comes back as the period ends."""
    remaining = lim["vm_hours_left"] * lim["vm_hour_dollars"]
    hours_left = max(0.25, ((lim["period_end"] or now) - now) / 3600)
    surplus_rate = (remaining - floor_rate() * hours_left) / hours_left
    extra = min(CAP_MAX - BASE, max(0, round(k * surplus_rate / WORKER_RATE)))
    return {"remaining": remaining, "hours_left": hours_left, "surplus_rate": surplus_rate, "allows": BASE + extra}


def burst_k(st, now):
    """K measured from the last K_HISTORY: the workers past BASE that the cap allowed on average while the controller
    ran, over the ones that actually ran on average over the whole time (bursts run only while it runs). BURST_K until
    the record covers a day with some burst allowed."""
    hist = [b for b in st.get("burst", []) if now - b[0] < K_HISTORY]
    st["burst"] = hist
    if not hist or now - hist[0][0] < 86400:
        return BURST_K, False
    on, used, allowed = (sum(b[i] for b in hist) for i in (1, 2, 3))
    if allowed <= 0 or on <= 0:
        return BURST_K, False
    k = K_RANGE[1] if used <= 0 else (allowed / on) / (used / (now - hist[0][0]))
    return min(K_RANGE[1], max(K_RANGE[0], k)), True


def record_burst(st, now, workers, cap):
    """Add this tick to the hourly record burst_k reads: [hour, seconds on, burst worker-seconds, allowed ones]."""
    dt = min(60.0, now - st.get("burst_at", now))
    st["burst_at"] = now
    hour = now - now % 3600
    hist = st.setdefault("burst", [])
    if not hist or hist[-1][0] != hour:
        hist.append([hour, 0.0, 0.0, 0.0])
    hist[-1][1] += dt
    hist[-1][2] += dt * max(0, workers - BASE)
    hist[-1][3] += dt * (cap - BASE)


def budget(st, now, workers):
    """The cap for this tick: re-read the limits and move the cap one worker toward what pace allows every BUDGET_EVERY
    seconds, so it does not flap. With no reading at all, BASE."""
    cap = st.get("cap", BASE)
    k, measured = burst_k(st, now)
    lim = st.get("pace_limits")
    if now - st.get("cap_at", 0) >= BUDGET_EVERY or lim is None:
        lim = limits(st)
        st["pace_limits"] = lim
        if lim is not None:
            p = pace(lim, now, k)
            cap += (p["allows"] > cap) - (p["allows"] < cap)
        else:
            cap = BASE
        st["cap_at"] = now
    cap = min(CAP_MAX, max(BASE, cap))
    st["cap"] = cap
    record_burst(st, now, workers, cap)
    if lim is None:
        return cap, None, "pace: no `boat limits` reading yet, cap at the base"
    p = pace(lim, now, k)
    line = (f"pace: ${p['remaining']:.2f} of VM time left, {p['hours_left']:.0f}h to the period's end, floor "
            f"${floor_rate():.3f}/h, surplus ${p['surplus_rate']:+.3f}/h, K {k:.1f}{'' if measured else ' (default)'} "
            f"| cap {cap} (budget allows {p['allows']}) | wallet ${lim['wallet']:.2f}"
            + (f" | boat limits failed, using the reading from {int(now - lim['at'])}s ago" if lim["stale"] else ""))
    return cap, p, line


def spawn(missing, burst=False):
    """Create `missing` workers, within the start allowance. About a share p of new VMs can reach the scheduler (boat's
    network blackholes some pairs of VMs, for good), so each round creates only as many as are still missing, waits for
    their WireGuard handshakes, deletes the ones that cannot reach the scheduler and makes the shortfall in the next
    round. No start is spent on a VM that worked and was then deleted; a round that falls short costs HANDSHAKE_WAIT."""
    st = load_state()
    st["creates"] = [t for t in st["creates"] if time.time() - t < 86400]
    st["extra_spend"] = [(t, d) for t, d in st.get("extra_spend", []) if time.time() - t < 30 * 86400]
    kept = []
    for round_number in range(1, ROUNDS + 1):
        now = time.time()
        if st.get("backoff_until", 0) > now:
            log(f"  boat refused a start recently; waiting {int(st['backoff_until'] - now)}s")
            break
        want = missing - len(kept)
        make, price, extra = start_allowance(want, st)
        if make < want:
            log(f"  start budget: wanted {want} starts, may use {make}"
                + (f", {extra} of them at ${price:.2f}" if extra else ""))
        if make == 0:
            break
        peers, _ = wg_peers()
        ips = free_ips(peers)
        if len(ips) < make:
            log("  not enough free worker addresses")
            break
        hub = hub_ipv6()
        names = [f"nl-w-{'b' if burst else ''}{int(now)}-{round_number}-{i}" for i in range(make)]
        ttl = BURST_TTL if burst else WORKER_TTL
        if DRY:
            log(f"  [dry-run] would create {make} workers {names[0]}..")
            break
        with tempfile.TemporaryDirectory() as tmp, cf.ThreadPoolExecutor(max_workers=make) as pool:
            ids = list(pool.map(lambda a: create_candidate(a[0], a[1], hub, tmp, ttl), zip(names, ips)))
        created = {n: i for n, i in zip(names, ids) if i}
        st["creates"] += [now] * len(created)
        if extra:
            st["extra_spend"] += [(now, price)] * min(extra, len(created))
        if len(created) < make:
            st["backoff_until"] = now + 600        # boat refused some: its limits are reached
        log(f"  round {round_number}: created {len(created)}/{make}; waiting up to {HANDSHAKE_WAIT}s for handshakes")
        good = []
        deadline = time.time() + HANDSHAKE_WAIT
        while created and time.time() < deadline and len(good) < len(created):
            time.sleep(3)
            peers, age = wg_peers()
            good = [n for n in created if n in peers and age.get(peers[n][0]) is not None and age[peers[n][0]] < 120]
        for n, i in created.items():
            if n not in good:
                boat("delete", "--yes", i)
                run(["/usr/local/bin/remove-peer.sh", n])
        if created:
            st["p"] = 0.7 * st.get("p", 0.5) + 0.3 * (len(good) / len(created))
        kept += good
        save_state(st)
        log(f"  round {round_number}: {len(good)} of {len(created)} reached the scheduler, deleted the rest; "
            f"{len(kept)} of {missing} made; reachable share now {st['p']:.2f}")
        if len(kept) >= missing or len(created) < make:
            break
    save_state(st)


def reap_and_extend(workers, active, wanted, kept):
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
        # A worker that runs out mid-build takes its running actions with it, and the scheduler gives them to another
        # only after its worker timeout (30 s), so while a build runs every connected worker within the cap is kept;
        # past the target they lapse once builds stop. Past the cap the budget wins: those run out their lifetime.
        if w["name"] not in wanted and w["name"] not in kept:
            continue
        burst = w["name"].startswith("nl-w-b")
        left = (parse_time(w.get("archiveAfter")) or 0) - time.time()
        if left < (BURST_TTL // 2 if burst else EXTEND_WHEN_LEFT):
            log(f"  extending {w['name']} ({int(left)}s left)")
            if not DRY:
                boat("extend", w["id"], "--ttl", str(BURST_TTL if burst else WORKER_TTL))


def idle_tails(state, now, active, queue, oldest, p, lim):
    """With nothing to spare (surplus_rate <= 0), delete the workers past BASE once the fleet has run nothing for
    IDLE_DELETE, instead of paying for the rest of their lifetime. Only while more than half of the hour's and the day's
    starts are left, since a replacement costs about 2 starts and 30-60 s; the oldest BASE keep their lifetime."""
    idle = not active or queue == (0, 0)
    state["idle_since"] = (state.get("idle_since") or now) if idle and queue is not None else None
    extra = oldest[BASE:]
    if not extra or p is None or p["surplus_rate"] > 0 or now - (state["idle_since"] or now) < IDLE_DELETE:
        return
    starts = lim["starts"]
    if lim["stale"] or any(starts[w]["remaining"] <= starts[w]["limit"] / 2 for w in ("hour", "day")):
        return
    for w in extra:
        log(f"  idle {int(now - state['idle_since'])}s with no surplus: deleting {w['name']}")
        if not DRY:
            boat("delete", "--yes", w["id"])
            run(["/usr/local/bin/remove-peer.sh", w["name"]])


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
    st = load_state()
    cap, p, pace_line = budget(st, now, len(workers))
    save_state(st)
    queue = demand() if active or workers else None
    target = min(cap, max(BASE, state.get("target", BASE))) if active else BASE
    note = ""
    if active and queue is not None:
        queued, executing = queue
        needed = math.ceil((queued + executing) / SLOTS)
        note = f" | queued {queued}, executing {executing} (needs {needed} at {SLOTS} each)"
        # Work that has waited: add one worker, at most every UP_EVERY seconds. A burst of a build is often over in a
        # minute, sooner than a new worker would help, so the fleet grows one at a time while the queue lasts. Only once
        # the target's workers are all connected: work queued for want of the ones not made yet (no starts left, or
        # still booting) is no reason to want more, and raising the target then only spends starts.
        state["queued_since"] = (state.get("queued_since") or now) if queued > 0 else None
        if (queued > 0 and now - state["queued_since"] >= UP_AFTER and now - state.get("last_up", 0) >= UP_EVERY
                and target < cap and not starting and len(alive) >= target):
            target = min(cap, target + 1)
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
    log(f"  {pace_line}")
    oldest = sorted(alive + starting, key=lambda w: parse_time(w.get("createdAt")) or 0)
    wanted = {w["name"] for w in oldest[:target]}
    reap_and_extend(workers, active, wanted, {w["name"] for w in oldest[:cap] if w in alive})
    idle_tails(state, now, active, queue, oldest, p, st.get("pace_limits"))
    if active:
        missing = target - len(alive) - len(starting)
        if missing > 0:
            log(f"  need {missing} more workers")
            spawn(missing, burst=len(alive) + len(starting) >= BASE)
    elif not workers and now - state["last_active"] > SCHEDULER_IDLE_STOP:
        log("idle past the limit with no workers: stopping the scheduler")
        if not DRY:
            # Boat's snapshot keeps no /tmp, so the small tier lives on only as its pack: pack it first, or what was
            # uploaded since the last pack is gone, though clients may still take the server to hold it.
            code, _, err = run(["systemctl", "start", "nl-small-pack.service"], timeout=900)
            if code != 0:
                log(f"  packing the small tier failed, stopping anyway: {err.strip()}")
            run(["boat", "stop", "self"], env={"HOME": "/home/user"})


def main():
    global DRY, LIMITS_FILE
    ap = argparse.ArgumentParser()
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--dry-run", action="store_true", help="decide and log, but create, delete, extend and stop nothing")
    ap.add_argument("--force-active", action="store_true", help="act as if a build were running (testing)")
    ap.add_argument("--limits", metavar="FILE", help="read this `boat limits --json` instead of asking boat (testing)")
    a = ap.parse_args()
    DRY = a.dry_run
    LIMITS_FILE = a.limits
    state = {"last_active": time.time()}    # a freshly woken scheduler gets the full idle time
    log(f"nl-controller: {BASE}-{CAP_MAX} x {WORKER_TYPE} workers, worker ttl {WORKER_TTL}s, scheduler idle stop {SCHEDULER_IDLE_STOP}s, "
        f"start price limit ${MAX_START_PRICE:.2f}, extra-start budget ${EXTRA_START_BUDGET:.2f}/month, "
        f"floor ${floor_rate():.3f}/h ({BASE} workers for {FLOOR_HOURS:g}h a day and the scheduler)"
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
