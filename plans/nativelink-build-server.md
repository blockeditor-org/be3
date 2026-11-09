Depends on: none.

# NativeLink build server on boat: handoff

This is the status of the pull request that makes `nativelink` the default build server. Delete this file when the work lands.
guides/build_server.md has the lasting description; this has what is running, what is not done, and what to do next.

## What the pull request does

- `crates/be-tool-bootstrap`: a `nativelink` server, now the default. buck2 reaches the scheduler through a tunnel: the
  scheduler VM serves a WebSocket bridge (`nl-tunnel.py`) on a port boat hosts behind a token, and
  `scripts/internal/re-relay` (`ensure-tunnel`, `tunnel.go`) carries each of buck2's TCP connections to it inside one
  WebSocket. No root, WireGuard or IPv6 on the client, and it works through an HTTPS proxy, so it works on GitHub-hosted
  runners and in Claude Code cloud sessions. The bootstrap wakes a stopped scheduler with a boat key that can only read
  and resume it.
- `.github/workflows/ci.yml`: the `ci` job (still on `ubuntu-24.04`) uses `BE3_BUILD_SERVER=nativelink`, `BE3_NL_URL`,
  `BE3_NL_KEY` and `BE3_BOOTSTRAP: cargo` (the checkout's own bootstrap, because the pinned one would refuse the new
  server's name). `broad.yml` still names `api.be3.pfg.pw` and is unchanged.
- `scripts/internal/nativelink/`: the scheduler (NativeLink config, units, the controller, the tunnel bridge, the
  WireGuard scripts the workers use) and the worker (config, bootstrap script) exactly as running.

## What the owner must do before CI can pass

CI fails until all of this is done (it fails with the bootstrap's message about the missing address):

1. **Expose the tunnel.** On the scheduler VM (`nl-sched-small`, boat sandbox bx_59kjmqmz) run `host 8099 --localhost`, then
   `host url 8099`, which prints `https://NAME-8099.on.boat.dev/?_token=TOKEN`. That whole string, token included, is the
   repository secret `BE3_NL_URL`. (The author's tooling was not allowed to expose a port, so this was not done and the
   path through boat's edge is untested: see Not tested.) Treat the token like the gateway's header: it lets a build use
   the server and run code on its workers.
2. **Make the wake key** and store it as the repository secret `BE3_NL_KEY`:
   `boat api-key create be3-ci --ttl 90d --env <id of the nl-scheduler environment> --actions sandbox.read,sandbox.resume`
   It can only read and resume the scheduler. The key already made, `nl-wake-upd2`, can do that and more; a narrower one is better.
3. Optionally `boat billing extra-starts on`: boat's start limits (60 an hour, 200 a day on the $20 plan) count every VM
   the controller creates, deleted ones too, and a cold fleet is up to 8 starts.

The address of a new scheduler differs, so step 1 and the secret are redone if the scheduler is ever recreated.

## What is running now

- Scheduler `nl-sched-small` (small, environment `nl-scheduler`): NativeLink (`nativelink-server.service`), the
  controller (`nl-controller.service`), the bridge (`nl-tunnel.service`, 127.0.0.1:8099) and the WireGuard hub `wg-sch`
  (10.100.0.1) that the workers use. Its boat key for the workers is in `/etc/nativelink/boat-workers.key` (copied by hand:
  it should be a secret of the `nl-scheduler` environment).
- Workers are created by the controller in environment `nl-worker`, named `nl-w-*`, and expire 30 minutes after the last build.
- `nl-sched-default` is an older scheduler, stopped; delete it. `nl-client` is the test client.
- The first NativeLink server, on the VM the author worked in, is still running with the old `wg-nl` network and holds
  secrets (see Cleanup).

## Tested

- The tunnel on loopback, Go client against the Python bridge: frame sizes at every length boundary (125, 126, 127, 65535,
  65536, 65537), 5 MB with an identical hash, 25 concurrent connections, 45 idle seconds across keepalive pings, a wrong
  address failing at start with a message, a version change replacing the tunnel.
- The whole chain with the real bootstrap and buck2, using `boat forward` to reach the bridge instead of a public
  address: a cached build (12 of 12 from cache) and a one-file change (3 actions run remotely, 41 cached), after which the
  controller saw the build from the tunnel's connections and kept its workers alive.
- `./scripts/verify` passes against this server. The controller's spawn, trim and extend logic, and a cold-resumed scheduler.

## Not tested

- **Boat's edge**: whether its proxy lets a long-lived WebSocket carrying HTTP/2 through, with the token cookie, and
  whether it cuts idle or long connections. The tunnel pings every 20 s. If connections are cut, buck2 reports an
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

A one-file change to be-lib-geometry with 4 default workers reading Bunny directly: 250-350 s with a fresh fleet, 190-230 s
with a warm one. A `small` scheduler used at most 0.7 of its 2 vCPU and under 1 GB. 2 large workers were as fast or faster
than 4 default ones when warm (141-190 s) but a single run each, so the difference is not established.
