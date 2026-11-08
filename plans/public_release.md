# Public release

Depends on: plans/daily_driver.md.

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

## Server

- App tokens (sessions of kind `app`) never expire; only browser sessions
  do. Done: an expiry with sliding renewal.
- A health endpoint and a config file in place of flags.

## Web offline

- The service worker (`crates/be-host/web/sw.js`) keeps every build's files
  it ever served; nothing prunes the old ones.
