# Public release

What the daily driver plan (`plans/daily_driver.md`) leaves out because one
person using the host on their own devices does not need it, but other people
using it would. Each item says what "done" looks like.

## Keys and accounts

- **Keys in the OS keystore.** The account's identity secret and the session
  token sit in plaintext host state (`host.sqlite3`, `HostStateStore`). Done:
  Keychain, Android Keystore, libsecret or DPAPI, and non-extractable
  WebCrypto keys in IndexedDB on web.
- **Pairing names the device by its OS only** ("linux", from
  `std::env::consts::OS` in `be-host-stack/src/keys.rs`). Done: a device name.

## Isolation between accounts

- `MissingObjects` answers for any hash a signed-in account names, so it
  tells anyone whether the server holds an object. `GetObject`
  already checks access (`may_read_object`). Done:
  scoped to the objects the reader may see.

## Server

- App tokens (sessions of kind `app`) never expire; only browser sessions
  do. Done: an expiry with sliding renewal.
- The portal counts its per-address limits (password hashing, sign-in
  lockouts, mail links) by the connection's peer address
  (`serve_auth` in `be-server/src/http.rs`), which behind a proxy is the
  proxy's. Done: the client's address from a forwarded header the server
  trusts only from its proxy.
- A health endpoint and a config file in place of flags.

## Web offline

- The service worker (`crates/be-host/web/sw.js`) keeps every build's files
  it ever served; nothing prunes the old ones.
