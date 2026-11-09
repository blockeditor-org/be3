Depends on: none.

# NativeLink build server on boat (work in progress)

Status as of the first push: the server side is built and tested; the be3 client side (bootstrap server mode, CI, docs) is
being written. This file is the handoff; delete it when the work lands.

## What exists

- `scripts/internal/nativelink/`: everything the scheduler and the workers run (NativeLink configs, systemd units, the
  worker bootstrap, the controller, the WireGuard peer scripts). Nothing in it is a secret.
- Architecture: a small `nl-scheduler` VM (boat environment `nl-scheduler`) runs NativeLink (CAS, action cache,
  scheduler) and the controller. Workers are 4 default-size VMs in boat environment `nl-worker`, created by the
  controller while a build runs, kept alive by extending their lifetime, and left to expire after 30 idle minutes.
  Workers read large objects (16 KB and up) straight from the Bunny zone `be-nativelink-s3` with a read-only key; the
  scheduler keeps small objects and the action cache in /tmp and persists them as one packed Bunny object.
  Clients and workers reach the scheduler over WireGuard (10.100.0.0/24; the scheduler is 10.100.0.1).
- Measured: a one-file change to be-lib-geometry takes about 250-350 s with a fresh worker fleet and 190-230 s with a
  warm one (4 default workers).

## Known constraints

- Boat places new VMs at random and about half cannot reach the scheduler over IPv6 (a boat network issue; it is stable
  per VM). Workers are over-provisioned 2x and trimmed by WireGuard handshake. A client that cannot reach the scheduler
  must fail with a clear message; re-running on a new VM is the workaround until boat fixes it.
- GitHub-hosted runners are IPv4-only and cannot reach the scheduler. CI jobs that build must run on boat
  (`runs-on: boat-small`), which needs the boat CI GitHub App installed.
- Claude Code cloud sessions cannot run WireGuard and are not supported yet (a WebSocket tunnel in re-relay is the plan).

## Next steps

See the pull request description for the current list.
