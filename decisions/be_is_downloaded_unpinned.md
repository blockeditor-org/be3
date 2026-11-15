# `be` is downloaded unpinned and trusted from the CDN

`./scripts/be` reads the build id from `https://be3.pfg.pw/live/be-release`, which each deploy
writes after it swaps the server, and downloads that build's `be` and `build-info.json` from
`data/BUILD/be-TRIPLE/` with no hash or signature check (crates/be-tool-bootstrap/src/be.rs).
Nothing about `be` is in `scripts/internal/pins`.

## Why

- The server refuses a `be` from another build, so the `be` worth running is always the live
  one. A pin in the repository went stale with every deploy and needed a commit to catch up,
  and a deploy from a branch could not move main's pin.
- Deploying already trusts the Bunny zone with everything people download: the web app, the
  desktop downloads and the APK. Checking `be` against the same zone adds no trust boundary.

## Rejected

- **A hash in `be-release`.** It comes from the same zone as `be` itself, so whoever can change
  one can change the other. It would catch accidents (a stale cached object, a partly written
  upload) but not an attacker, and deploy already reads each `be` back and checks its hash
  before it writes `be-release`.
- **A signed `be-release`.** It would protect against a compromised zone, but needs a signing key
  held by deploys, its public half in the repository and a rotation story, for one tool that
  agents run against a server that checks its build anyway. Worth revisiting if the zone stops
  being trusted for the app's downloads too.
- **Keeping the pin, rewritten by deploy.** Deploy wrote into the checkout and needed a commit
  after every run, and the pin was wrong between a deploy and that commit landing.
