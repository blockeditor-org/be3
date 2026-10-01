# Public release

What the daily driver plan (`plans/daily_driver.md`) leaves out because one
person using the app on their own devices does not need it, but other people
using it would. Each item says what "done" looks like.

## Keys and accounts

- **Warn when a workspace has no sealed copy.** A device seals its workspace
  key to the recovery phrase the first time it connects. If that fails, the
  error only reaches the debug window, and the device's own copy is the only
  one. Done: the app shows when the server holds no sealed copy for you, and
  retries.
- **Password reset.** The sealed copy is only handed to a signed-in account,
  so a lost password with a kept phrase still locks you out. Done: a reset
  path (email, or at least a `be-server` command for the operator), which
  leaves the sealed keys as they are.
- **Keys in the OS keystore.** Workspace keys and the session token sit in
  plaintext app state (`app.sqlite3`, or localStorage on web). Done: Keychain,
  Android Keystore, libsecret or DPAPI, and non-extractable WebCrypto keys in
  IndexedDB on web.
- **Key rotation.** A workspace has one key for its whole life, and sealed
  objects carry no key id. Done: key epochs in the object prefix (phase 2 of
  the daily driver plan), and rotating the key when a member leaves.
- **Invited members get a sealed copy late.** A member's copy is sealed only
  when a device holding the key next connects after they saved their phrase.
  Until then they can only pair. Done: sealing for a member as soon as they
  accept, from any connected device holding the key.
- **Pairing names the device by its OS only** ("linux"). Done: a device name.

## Isolation between accounts

Registration is closed for personal use, so only one account exists. Before
anyone else can sign in:
- Scope `GetObject`, `PutObject` and `MissingObjects` to the open workspace.
  Today any account can read an object whose hash it knows.
- Check access on `Watch`, `ClaimOwnership`, `Relay` and the other session
  messages.
- Key the session registry and watchers by (workspace, block). Today they are
  keyed by block UUID alone, and the client picks the UUID
  (`be-server/src/lib.rs`, `sessions.rs`).
- Invitations bind to an email with no verification.
- Logging out leaves the connection in the hub's workspace and watcher maps.

## Server

- Tokens never expire and cannot be revoked except by logging out on the
  device that holds them. Done: an expiry with sliding renewal, change
  password, and "sign out other devices".
- Sign-in lockout is per email. Done: per-IP limits too, from the forwarded
  address be-server already reads.
- No storage quota per account, and objects uploaded but never published are
  never collected.
- A health endpoint and a config file in place of flags.

## Web offline

- Native and Android are what the daily driver uses.
- Web offline needs an IndexedDB object store and refs database. `ObjectStore`
  is synchronous, so that needs an in-memory front with async write-behind.
