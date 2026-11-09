Depends on: none.

# NativeLink build server on boat: handoff

This is the status of the pull request that makes `nativelink` the default build server. Delete this file when the work lands.
guides/build_server.md has the lasting description; this has what is running, what is not done, and what to do next.

## What the pull request does

- `crates/be-tool-bootstrap`: a `nativelink` server, now the default. buck2 reaches the scheduler through
  `scripts/internal/re-relay -http1`, which sends each gRPC call as HTTP/1.1 to boat's public hosted port, where Caddy
  checks a bearer token and forwards it to NativeLink. No root, WireGuard or IPv6 on the client, and it works through an
  HTTPS proxy, so it works on GitHub-hosted
  runners and in Claude Code cloud sessions. The bootstrap wakes a stopped scheduler with a boat key that can only read
  and resume it.
- `.github/workflows/ci.yml`: the `ci` job (still on `ubuntu-24.04`) uses `BE3_BUILD_SERVER=nativelink`, `BE3_NL_URL`,
  `BE3_NL_KEY` and `BE3_BOOTSTRAP: cargo` (the checkout's own bootstrap, because the pinned one would refuse the new
  server's name). `broad.yml` still names `api.be3.pfg.pw` and is unchanged.
- `scripts/internal/nativelink/`: the scheduler (NativeLink config, units, the controller, Caddy, the
  WireGuard scripts the workers use) and the worker (config, bootstrap script) exactly as running.

## What the owner must do before CI can pass

CI fails until all of this is done (it fails with the bootstrap's message about the missing address). Done already: the
wake key (`sandbox.read,sandbox.resume` on `nl-scheduler`), and the controller's key with `account.read` as the
`nl-scheduler` environment's secret file `nl-controller.key`, logged in and restarted on the scheduler.

1. **Caddy's token**: done (the secret file `nl-tunnel.env`, delivered with `boat env upgrade nl-scheduler`).
2. **Deploy Caddy** to the scheduler (`bx_59kjmqmz`) in place of the WebSocket bridge, from a checkout of this branch:
   `tar czf - -C scripts/internal/nativelink scheduler worker | boat ssh bx_59kjmqmz 'rm -rf /tmp/nl && mkdir -p /tmp/nl && tar xzf - -C /tmp/nl'`,
   then run the Caddy lines of `setup-scheduler.sh` there, install `nl-caddy.service`, and stop and remove `nl-tunnel`.
3. **Expose it publicly**: `host 8099 --localhost --public` (done). Check that `curl -i https://NAME-8099.on.boat.dev/`
   answers `401` from nl-caddy.
4. **Secrets**: the repository secret `BE3_NL_URL` is `https://NAME-8099.on.boat.dev/?_token=` and Caddy's token;
   `BE3_NL_KEY` is the wake key. In a cloud session, network secrets carry both instead (guides/build_server.md).
5. Optionally `boat billing extra-starts on`: boat's start limits (60 an hour, 200 a day on the $20 plan) count every VM
   the controller creates, deleted ones too, and a cold fleet is up to 8 starts.

The address of a new scheduler differs, so step 3 and the secret are redone if the scheduler is ever recreated.

## What is running now

- Scheduler `nl-sched-small` (small, environment `nl-scheduler`): NativeLink (`nativelink-server.service`), the
  controller (`nl-controller.service`), Caddy (`nl-caddy.service`, 127.0.0.1:8099) and the WireGuard hub `wg-sch`
  (10.100.0.1) that the workers use. Its boat key for the workers is the environment's secret file `~/nl-controller.key`.
- Workers are created by the controller in environment `nl-worker`, named `nl-w-*`, and expire 30 minutes after the last build.
- `nl-sched-default` is an older scheduler, stopped; delete it. `nl-client` is the test client.
- The first NativeLink server, on the VM the author worked in, is still running with the old `wg-nl` network and holds
  secrets (see Cleanup).

## Tested

- Caddy's front locally (the pinned 2.11.7 and this Caddyfile, against an HTTP/2 backend): no token, a wrong one and an
  empty `Bearer ` get 401, also with the variable unset; the right token reaches the backend as HTTP/2 without its
  `Authorization` header. The token stays a runtime placeholder, so it is never in Caddy's adapted config.
- From a cloud session: the proxy adds the token on `on.boat.dev`, and an HTTP/1.1 gRPC request crosses boat's edge to
  the scheduler; the WebSocket upgrade does not (the session's proxy drops it), which is why the tunnel went.
- Earlier, with the WebSocket tunnel and `boat forward` instead of the public address: a cached build and a one-file
  change through the real bootstrap and buck2, with the controller keeping its workers alive.
- `./scripts/verify` passes against this server. The controller's spawn, trim and extend logic, and a cold-resumed scheduler.

## Not tested

- **Boat's edge**: whether it keeps long streaming calls (`Execute`, `ByteStream`) open as HTTP/1.1, and whether it
  cuts idle or long connections. The relay repeats an `Execute` that breaks off. If connections are cut, buck2 reports an
  infrastructure error and the wrapper retries.
- A run from a real GitHub-hosted runner and from a Claude Code cloud session (the proxy path is net/http's CONNECT).
- A **cold wake-up**: boat keeps a stopped VM for a while; after that a resume is a new machine at a random place and may
  not be reachable by its workers. The wake key has `sandbox.fork`; the client does not yet fork a fresh scheduler.

## Follow-ups

- Move the browser tests onto the workers (they are `local = True` in `buck/program/drive.bzl`): Node, Playwright's
  Chromium and Chromium's libraries as a pinned sysroot, as the host tests do with Lavapipe. The client then needs no browser.
- Use GitHub-hosted runners' free compute as extra workers for buck2 actions in CI.
- Fork the scheduler when a resumed one is unreachable; read the hosted address from boat's API so the wake key alone is enough.

## Cleanup after the cutover

- Stop and remove NativeLink and `wg-nl` from the VM the author worked in; delete `/etc/nativelink`, `~/nativelink`,
  and the keys under `~/.config/be3`.
- Rotate both Bunny keys of `be-nativelink-s3` (they were printed into a session transcript) and the boat key `boat-secret`.
- Revoke the boat keys `nl-wake` and `nl-wake-upd`; revoke or narrow `nl-wake-upd2`.
- About 340k small objects from the first cold run are still in the Bunny zone under `cas/` and `ac/`.

## Measured

One build alone, a one-file change to be-lib-geometry with 4 default workers reading Bunny directly: 250-350 s with a fresh
fleet, 190-230 s warm. A new client VM with a fresh clone and daemon and a fully warm cache: 12 s to `verify` after about 80 s
of setup (clone, the cargo bootstrap, buck2 and the relay). 8 builds at once from 3 VMs: 860-888 s each, scaling 4 to 9
workers on CPU. A `small` scheduler used at most 0.7 of its 2 vCPU and under 1 GB. 2 large workers were as fast or faster
than 4 default ones when warm (141-190 s), single runs.

## Scaling on the queue (collector deployed; scaling under load not yet measured)

The controller in this branch scales on NativeLink's queue depth, as NativeLink's docs advise, and not on CPU: worker CPU
cannot say whether work is waiting (the 8-build run above averaged 45-72% busy and crossed the scale-up threshold once).
NativeLink exports `execution.active.count` by `execution.stage` over gRPC OTLP, an OpenTelemetry collector
(`scheduler/otelcol.yaml`, `nl-otelcol.service`, downloaded by setup-scheduler.sh against GitHub's published digest) serves it
as `nativelink_execution_active_count`, and the workers take a fixed number of actions at once (`max_inflight_tasks` in
worker-direct.json5) so that extra work queues. The controller scales to `ceil((queued + executing) / slots)` with burst
workers that expire.

Deployed on `nl-sched-small` (otelcol-contrib 0.165.0, sha256-checked). Verified: metrics flow, and a test build showed
`queued=1` then `executing=1` (34 s build, 2 remote actions). Not done: no multi-build run happened with the new controller
(the day's start allowance was below the controller's reserve), so scale-up under real queueing is unobserved. Next: repeat
the 8-build run (`scripts/internal/nativelink/bench/run-timings.sh`) when about 50 starts are free, compare with the CPU
numbers above, and tune `max_inflight_tasks` (8 per 4 vCPU is a guess).
