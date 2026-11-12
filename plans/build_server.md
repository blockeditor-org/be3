# The NativeLink build server: what is left

Depends on: none.

The NativeLink server on boat is the default build server and CI's
(guides/build_server.md). Left:

- **Fork the scheduler when a resumed one is unreachable.** A scheduler boat
  resumes as a new machine can land where its workers cannot reach it; the
  wake key has `sandbox.fork`, but the client does not yet fork a fresh
  scheduler. Read the hosted address from boat's API too, so the wake key alone
  is enough. Done: a client whose scheduler's workers cannot reach it gets a
  working one without a person.
- **GitHub-hosted runners as extra workers** for buck2 actions in CI, using
  their free compute.
- **Boat's edge** is untested for long streaming calls (`Execute`,
  `ByteStream`) over HTTP/1.1 and for cutting idle or long connections; the
  relay repeats an `Execute` that breaks off. Done: a long build through the
  edge with no infrastructure retries, or the cut found and worked around.

## Cleanup after the cutover (owner's steps, not checked from the repository)

- Stop and remove the first NativeLink server and `wg-nl` from the VM its
  author worked in; delete `/etc/nativelink`, `~/nativelink` and the keys under
  `~/.config/be3`. Delete the stopped scheduler `nl-sched-default`.
- Rotate both Bunny keys of `be-nativelink-s3` (they were printed into a
  session transcript) and the boat key `boat-secret`.
- Revoke the boat keys `nl-wake` and `nl-wake-upd`; revoke or narrow
  `nl-wake-upd2`.
- About 340k small objects from the first cold run are still in the Bunny zone
  under `cas/` and `ac/`.
