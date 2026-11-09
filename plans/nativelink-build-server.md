Depends on: none.

# NativeLink build server on boat: handoff

This is the status of the pull request that makes `nativelink` the default build server. Delete this file when the work lands.
guides/build_server.md has the lasting description; this has what is running, what is not done, and what to do next.

## What the pull request does

- `crates/be-tool-bootstrap`: a `nativelink` server, now the default. It wakes the scheduler VM through boat's API,
  registers this machine's own WireGuard key (`nl-register-client.sh` on the scheduler, run with `exec`), brings up the
  `wg-be3` interface and gives buck2 `10.100.0.1:50052`. Tested end to end on a boat VM: from nothing in 3 s; a cold
  scheduler is resumed and registered (it was not reachable from that VM, and the error said so).
- `.github/workflows/ci.yml`: the `ci` job runs on `runs-on: boat` with `BE3_BUILD_SERVER=nativelink`, `BE3_NL_KEY` and
  `BE3_BOOTSTRAP: cargo` (the checkout's own bootstrap, because the pinned one would refuse the new server's name),
  and installs wireguard-tools. `broad.yml` still names `api.be3.pfg.pw` and is unchanged.
- `scripts/internal/nativelink/`: the scheduler (NativeLink config, units, the controller, the WireGuard scripts) and
  the worker (config, bootstrap script) exactly as running.

## What the owner must do before CI can pass

CI fails until all of this is done (it fails with the bootstrap's message about the missing key, or waits for a runner):

1. Install the boat CI GitHub App on the organization and give it this repository (`boat ci connect`). Until then the
   `ci` job queues for a runner that never comes. `boat-small` is 2 vCPU and 4 GB; the workflow uses `boat` (4 vCPU,
   8 GB) because the browser tests run Chrome on the runner.
2. Make the CI key and store it as the repository secret `BE3_NL_KEY`:
   `boat api-key create be3-ci --ttl 90d --env <the nl-scheduler environment's id> --actions sandbox.read,sandbox.resume,exec`
   **That key can run any command on the scheduler**, which holds the full-access Bunny key and the key that creates
   workers. The CI job runs pull request code with it in its environment. Same-repository pull requests only get it.
   The fix is a registration endpoint that takes only a public key (below).
3. Boat's start limits (60 an hour, 200 a day on the $20 plan) count every VM the controller creates, deleted ones too;
   a cold fleet is up to 8 starts. `boat billing extra-starts on` lets it pay per extra start.

## What is running now

- Scheduler `nl-sched-small` (small, environment `nl-scheduler`): NativeLink (`nativelink-server.service`), the controller
  (`nl-controller.service`) and WireGuard hub `wg-sch` (10.100.0.1). Its boat key for the workers is in
  `/etc/nativelink/boat-workers.key` (copied by hand: it should be a secret of the `nl-scheduler` environment).
- Workers are created by the controller in environment `nl-worker`, named `nl-w-*`, and expire 30 minutes after the last build.
- `nl-sched-default` is an older scheduler, stopped; delete it. `nl-client` is the test client.
- The first NativeLink server, on the VM the author worked in, is still running with the old `wg-nl` network and
  holds secrets (see Cleanup).

## Not done or not tested

- **Cold wake-up**: boat keeps a stopped VM for a while and a resume then gives the same machine; after that a resume
  is a new machine at a random place. Resuming a cold scheduler worked, but it landed where the test client could not
  reach it. The client should then fork a fresh scheduler (the wake key has `sandbox.fork`) until one is reachable, and
  delete the failed one. Not written.
- **Reachability**: about half of new boat VMs cannot reach the scheduler, in both directions, for good. A client on
  such a VM fails with a message; re-run on a new VM. Workers are handled by over-provisioning.
- Claude Code cloud sessions cannot run WireGuard and are unsupported: set `BE3_BUILD_SERVER=api.be3.pfg.pw` there.
  The plan is a WebSocket tunnel in `scripts/internal/re-relay` (boat's proxy blocks gRPC but carries WebSockets).
- The registration endpoint (a small HTTP service on the scheduler, reached through boat's hosted port) so CI's key does
  not need `exec`.
- Browser tests run on the client, not on the workers; with a small scheduler and 4 workers that tail is the slowest part.

## Cleanup after the cutover

- Stop and remove NativeLink and `wg-nl` from the VM the author worked in; delete `/etc/nativelink`, `~/nativelink`,
  and the keys under `~/.config/be3`.
- Rotate both Bunny keys of `be-nativelink-s3` (they were printed into a session transcript) and the boat key `boat-secret`.
- Revoke the boat keys `nl-wake` and `nl-wake-upd`; the wake key in use is `nl-wake-upd2`.
- About 340k small objects from the first cold run are still in the Bunny zone under `cas/` and `ac/`.

## Measured

A one-file change to be-lib-geometry with 4 default workers reading Bunny directly: 250-350 s with a fresh fleet, 190-230 s
with a warm one. A `small` scheduler used at most 0.7 of its 2 vCPU and under 1 GB. 2 large workers were as fast or faster
than 4 default ones when warm (141-190 s) but a single run each, so the difference is not established.
