# One way each in the tools

Depends on: none. Both remaining steps need a bootstrap release and a new pin (guides/bootstrap.md),
which only CI's publish-bootstrap job and its pin round make.

## Steps

1. **One acceptance of paintings** (needs a bootstrap release: CI's publish-bootstrap plus its
   pin round). be-tool-verify's `Run::paintings` copies a painting from
   `changed/` into `snapshots/` only when `used/NAME`'s hash matches the accepted file, and
   otherwise reports it stale; the bootstrap's `test.rs` `report()` (`./scripts/test --update`)
   copies without that check, so it can overwrite a painting edited after its test compared it.
   One `accept(package, record_dir) -> Accepted { changed, stale }` in a small library both
   use.
2. **The bootstrap's copies of the small helpers** (needs a bootstrap release: CI's
   publish-bootstrap plus its pin round). The other tools hash through be-lib-sha256 and run git
   through be-lib-git; crates/be-tool-bootstrap keeps its own: `download.rs`'s `hex()` and
   `sha256_file()` and `lock.rs`'s hex of its hasher become be-lib-sha256's `sha256_read` and
   `finish`, and `verify.rs`'s `git` closure in `changed_paintings` becomes be-lib-git's `git`
   with `-C`.
