# Moving the build server and agent sessions to boat.dev: status

Status of the feasibility spikes (2026-11-06). Nothing is implemented in the repo; this branch only
carries this file. Raw spike reports are in the session scratchpad and summarized here.

## Goal

Replace Namespace (and the other build servers `./scripts/buck` can pick) with NativeLink running on
boat.dev sandbox VMs, and run agent sessions and CI on boat too. The runner must be off when idle,
keep its cache across auto-stop and resume, and have its auto-stop deadline re-upped whenever a build
starts. Target cost: under $100-$200 per month with 12 h/day of build-server use.

Decisions made: boat becomes the default and everything except boat and `local` is deleted; the
personal boat account owns the VMs for now; 30 minute idle window.

## Proposed design (as of now)

- Runner: one `large` sandbox with a fixed subdomain (e.g. `be3-build`) running NativeLink
  (scheduler, CAS, action cache) as an enabled systemd service. Extra `default` (4 vCPU) workers can
  join its scheduler, to approach Namespace's usual 5-6 x 4 vCPU.
- Waker: the build helper's existing waker gets a `/build/boat` endpoint that resumes the VM, waits
  until NativeLink answers, resets the TTL to 30 minutes, and returns the buck2 config. Host it on an
  always-on machine (existing API host or a small OVH VPS), not on a boat VM.
- Idle handling: the waker re-ups the TTL on every `./scripts/buck` run, and a watchdog on the VM
  calls `boat extend self` while connections or actions are active, then `boat stop self` when idle.
- Clients (agent VMs, CI, laptops) reach the runner through a small local WebSocket-to-TCP bridge
  (see spike A), not directly.
- Agent sessions: `small` VMs created from a saved snapshot, with the build-server env vars set.
- CI: GitHub Actions on `runs-on: boat-small` or a persistent VPS; it is a thin client.

## Spike A: connecting buck2 to a boat VM (done)

- gRPC through `https://<sub>-<port>.on.boat.dev` FAILS. Cloudflare in front of boat returns 403 for
  any `content-type: application/grpc` request, on every port, with or without a valid token. Real
  buck2 hits the same 403 and falls back to a local build. Possibly fixable by boat support; not tried.
- WebSocket tunnelling through the same URL WORKS. A small Go bridge (WebSocket on the VM side to
  TCP, and the reverse on the client) carried gRPC with trailers and `grpc-status` intact. From this
  machine: about 24 ms per call, 60-100 MB/s on 100 MB messages. From a second sandbox: 40 ms,
  12-60 MB/s, about 21 MB/s aggregate over 16 streams. Idle streams survived 60 s, 300 s and 600 s;
  unary calls of 330 s and 700 s completed; 300 concurrent connections and 100 MB messages worked.
- Auth: `Cookie: _port_auth=<token>` works for HTTP and WebSocket; a wrong or missing token gets a 403
  with nothing leaked. The cookie as gRPC metadata could not be tested (blocked before auth).
- buck2 against a NativeLink (memory CAS/AC) through the tunnel: upload worked, and after clearing
  buck-out a second build was a 100% cache hit. Address must have no `http://` prefix and
  `[buck2] digest_algorithms = SHA256` is required.
- The raw sandbox IP is IPv6 only (IPv4 forwards SSH only), so GitHub-hosted runners cannot use it.
  `boat host <id> <port> --public` opens raw TCP on IPv6 with no TLS and no auth.
- Not tested: remote execution through the tunnel, disk-backed stores, a GitHub runner as client,
  reconnect/ping handling.

Recommended: extend `scripts/internal/re-relay` into the WebSocket bridge; the token stays in the
bridge process, buck2 points at `127.0.0.1` with `tls = false`.

## Spike B: cache across stop and resume (done)

- Resume is fast and a genuinely new machine: 5-7 s to `ready` regardless of data size; a resume with
  `--type large` took 7 s and grew the disk (79 GB to 147 GB). All 176,876 files (50.8 GB) matched
  sha256 after resume. Saved snapshot of a 51 GB sandbox: 3.2 s to save, 12 s to start a copy,
  4.2 min to hydrate.
- Hydration: 4.2 min for 50 GB when nothing read the files. Cold reads during hydration were poor:
  small files p90 0.8 s, p99 5 s, worst 22 s; large files about 18 MB/s (up to 67 s each). After
  hydration: 0.1 ms and 1.3 GB/s or more. Reading while hydrating appeared to slow hydration (about 4x,
  but the comparison used different data sizes). Whether hydration prioritizes the files a build needs
  is not known.
- Stop time depends on unsnapshotted data: about unsnapshotted GB / 0.055 GB/s. Fully caught up: 5 s.
  20 GB written just before: 141 s. Snapshots under heavy writes run back to back at 3.5-8 min each.
  Stops were never refused. /dev/shm staging stayed near 1 GB.
- Persistence: `/var/lib` and `/home/user` persist; `~/.cache` and `/tmp` do not. Hard links do NOT
  survive stop/resume (separate files afterwards). Rename, fsync, fallocate work. ext4, no reflink.
- systemd services come back, but in 2 of 3 restores a service failed on first start (files under
  `/var` not yet restored) and systemd gave up in about a second. `RestartSec=2` plus
  `StartLimitIntervalSec=0` fixed it (up about 4 s after `ready`).
- The disk is 147 GB with about 21 GB used by the base image, not the documented 111 GB.
- A fixed `subdomain` (set via PATCH with curl; the CLI has no flag) keeps the URL and token
  identical across stop/resume, and the host registration persists with no `boat host` call.
- From inside a sandbox, `boat info self`, `boat extend self --ttl 1800`, `--no-auto-stop`,
  `boat stop self` and PATCH of its own `ttlSeconds` all work; other sandboxes get 403. So the idle
  watchdog needs no extra key. `~/.boat/pre-stop` runs and is killed at about 60 s.
- Not tested: a very large write burst right before a stop, hard-link behavior under the real
  NativeLink worker, NativeLink itself.

Implications: cap the cache at about 40 GB, keep `cas`/`ac`/`tmp` on one filesystem under
`/var/lib/nativelink`, do not rely on hard links, add the systemd restart settings above. Whether
the waker should wait for `hydrated` is undecided (see open questions).

## Spike C: real builds, root vs docker, scaling (done, with gaps)

Full reports: `spike/c-report.md` and configs in `spike/c/cfg/` in the session scratchpad.

What worked:
- NativeLink v1.8.3 (musl release binary) runs on a `large` node as one systemd service (scheduler, CAS,
  action cache, local worker). The worker runs directly as root on the host with no docker image; the
  toolchains are hermetic, so the host only needs bash and coreutils.
- buck2 on a client, through `boat forward` in `local` mode, built `//crates/be-tool-bootstrap:bin` in
  about 9 s.
- `${ENV}` expansion in `worker-only.json5` works; all three workers reach main on 50051/50061 over IPv6
  (listener must bind `[::]`).

Cached rebuild (`//crates/be-host:native`, 6385 commands, 2.2 GiB output):
- 56 s with a fresh daemon and empty buck-out, 100% cache hits. After `systemctl restart nativelink`
  (about 4 s to listen), 57.8 s, 100% hits: the on-disk CAS and AC survive a restart.
- About 8 s is analysis, AC lookup and FindMissingBlobs; the rest is downloading outputs.

Scaling (`//third-party/rust/...`, 12090 commands, forced re-execution, 0% cache hits):

| Topology | vCPU | Wall time | Actions/s | Speedup |
|---|---|---|---|---|
| main alone | 8 | 555 s | 21.8 | 1.00 |
| main + 2 workers | 16 | 441 s | 27.4 | 1.26 |
| main + 3 workers | 20 | 533 s | 22.7 | 1.04 |
| 3 workers, main coordinator only, disk | 12 | 1362 s | 8.9 | 0.41 |
| same, worker data on tmpfs | 12 | 686 s | 17.6 | 0.81 |

Bottlenecks:
- Main's NIC (0.1-6 MB/s) and CAS disk are not the limit. A coordinator-only main sits at 8-18% CPU.
- `default` workers are limited by their own disk (about 350 IOPS, 13-20 ms await, 35-45% iowait).
  Putting worker CAS and work dir on a 5 GB tmpfs halved the time, after which workers idle at about
  70%, starved by input fetches over the 10 ms RTT.
- Three disk-backed `default` workers did not beat the `large` node alone.
- Worker `directory_cache` (30 GB) was much worse (system time 48%, 80k evictions); aborted.
- The CAS hit a 60 GB cap during the full `//crates/...` build and evicted blobs later actions needed
  ("action input missing from CAS"). The full closure needs more than 60 GB, so the cap is 100 GB for
  that workload (the 40 GB advice from Spike B must be revisited against the 147 GB disk).

Earlier cold `//crates/...` run (72 min, stopped): about 6 actions/s, CPU 30-50% system time plus iowait,
peak RAM 9.9 GB of 16 GB, no OOM kills.

Other findings: actions get no `PATH` (set it in `additional_environment`); the knob is
`max_inflight_tasks`, not `max_action_executions`; one test action aborted with "buffer overflow
detected" on the host glibc (not investigated); the first `be-host:native` run showed 0% cache hits
(unexplained).

Not tested: tuning `max_inflight_tasks` and worker fetch settings, `directory_cache` on tmpfs workers,
the docker worker with the pinned image, a non-root worker.

Recommended topology: start with one `large` node alone (about 22 actions/s here; warm incremental
builds are cache hits, 56 s for a full-output rebuild). Scaling out with plain disk-backed `default`
workers is not worth it. Matching Namespace's 5-6 x 4 vCPU would need a `large` coordinator plus 3-4
workers with RAM-backed data, about $0.18-0.22/h, and that was untuned and did not beat main alone. The
`xlarge` option (16 vCPU on one node) is the better thing to test next, since the bottleneck is
per-node disk and network round trips, which a single bigger node avoids.

State left behind: the four `spike-c-*` sandboxes (`bx_t9cps9kw` main with a large cache, workers
`bx_ss3wtr4v`, `bx_uwee6zdk`, `bx_2v35s2ee`) are running with TTL to about 08:09 UTC. Delete them when
done.

## Cost estimate (30 days, 12 h/day = 360 h)

| | `large` runner | `xlarge` runner |
|---|---|---|
| Runner | about $26 (+ about $1 idle tail) | about $72 (+ about $3) |
| Agents, `small`, 4-8 concurrent | $26-52 | $26-52 |
| CI on `boat-small` | under $5 | under $5 |
| Waker host | $0-5 | $0-5 |
| Total | about $55-90 | about $105-135 |

Rates: small $0.018/h, default $0.036/h, large $0.072/h, xlarge $0.20/h. The plan price is credited
against usage; xlarge needs the $100/month plan. Extra `default` workers add $0.036/h each while they
run (about $13 per worker for 360 h). All of this is under $200; xlarge around the clock would be
about $146 for the runner alone, so auto-stop matters.

## Open questions

1. Should the waker wait for `hydrated`? Measured: about 4 min for 50 GB, cold reads slow. Proposed
   instead: return once NativeLink answers and warm the hot set (toolchain and worker-input blobs)
   first. Needs one measurement: identical build immediately after resume vs after hydration.
2. Remote execution through the WebSocket tunnel and from a GitHub runner is untested.
3. The existing NativeLink servers' config (`api.be3.pfg.pw`, `blocks.pfg.pw`) is not in the repo.
4. Worker isolation: actions run arbitrary PR code; the worker should not be able to read the VM's
   boat credentials (see spike C for what is needed).
5. The bootstrap is a prebuilt, pinned release: land the new server code first, flip the default after
   CI pins the release, and only then turn Namespace off.
