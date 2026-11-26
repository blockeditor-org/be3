# Accounts have no passwords

A remote account signs in, registers and recovers by a code mailed to its email,
or signs in with a passkey or an OIDC provider beside it (guides/blocks/server.md,
"Accounts"). be-server keeps and hashes no password, and has no
lockouts or reset flow.

## Why

- Passwords were the costliest part of the server's account code: the hashing
  pool and its per-address queue, (email, address) and per-address lockouts, the
  per-email slowdown and reset caps. Credential stuffing and reuse stay a risk
  however well that is done.
- An emailed code is enough because an account holds no content keys: workspace
  keys are sealed to the recovery phrase, so taking an account over gets
  metadata and memberships, never plaintext or a signature.
- Codes go to the server log until mail delivery lands
  (plans/deferred/mail_delivery.md), which is only a little worse than the
  verification and reset links the log already held.

## Rejected

- Keeping passwords as a fallback: it keeps all of that machinery for no gain,
  since email recovery already overrides any other credential and approving a
  device code can be phished whatever signed in to the portal. A device with no
  browser signs in by opening the URL on another device, so none needs one.
