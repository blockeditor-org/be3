# Crypto audit: fixes

Depends on: none. Items 6, 9 and 10 touch plans/signed_access.md (noted on
each); do them with it or after it.

An audit of the key and crypto code for security and simplicity, against
guides/blocks/keys.md: be-block-keys, the seal and sign paths in
be-block-client and be-host-stack, escrow, recovery and rotation, contacts and
pairing, sign-in in be-server, the vault, backups, web push and the signed
records. Each finding names the attacker, what they do and what they gain.
Paths are as of a72f0b3f3. The server deciding who may hold a block's keys is
plans/signed_access.md's and is not repeated here.

## Who has what

Each item is marked when it is taken and when it lands (the fixing commit).

- 1: session_01UbiTY1dyaLgrBaJHjjZcSE, fixed on fix/never-seal-to-the-server
  (948703c94), ready to land.
- 2, 3: session_01UbiTY1dyaLgrBaJHjjZcSE, designed in
  plans/keys_bound_to_blocks.md. Of 3, the current key (8f2ea3f9e) and the
  pointer reads (46cb4c406) are on fix/keys-bound-to-blocks, in review; reused
  parts, escrow out of the keyring, and 2 follow.
- 7: session_01UbiTY1dyaLgrBaJHjjZcSE, fixed on fix/chat-deletion-author
  (46b45f634), ready to land.
- 10, and the overlapping parts of 6 and 9: the signed access builder
  (session_01UbiTY168uiE6R7Qo8b581U).
- 8, 9: session_01UbiTY4vhzKmTTZzb4YSfzd, on claude/audit-8-9; 9's revoking
  of an agent's signed Grant records (after signed access step 3b) is the
  signed access builder's.
- 4, 5: to be assigned.

## Security issues

### High: the server alone reads or forges content

1. **The server's own account is a sealing target.** `KeyBook::contact`
   (be-block-client keys.rs:194) answers the server's key for the server's
   account, so it can open the settings key the server seals (`accepts`). The
   sealing paths use the same lookup: key work's `unsealed` loop
   (peer/rotation.rs:49) seals any trusted key to any account `contact`
   answers, and `make_escrow` (rotation.rs:165) seals the escrow secret to any
   member the server lists as an administrator that `contact` answers. Attack,
   by the server alone: push `key-work-changed` listing each scope key as
   unsealed for the server's account, and every device seals its trusted scope
   keys to the server; or list the server account as an administrator with an
   escrow rotation due, and an administrator's device mints an escrow, seals it
   to the server and escrows every trusted scope key under it, which every
   member then follows. Fix: two lookups, one for whose seal is accepted (the
   server's, for the settings key only) and one for whom to seal to (never the
   server).
2. **An administrator trusts the settings key, which the server holds.** An
   administrator's device mints the organization settings block
   (`ensure_organization_settings`, peer.rs:629), so its scope key is trusted
   there, and seals that key to the server (`share_with_server`). A row
   wrapped under a trusted key opens trusted (`KeyBook::unwrap`, keys.rs:589;
   `opened` checks only the block the server also writes). Attack, by the
   server: store a row `{block: X, kind: data, epoch: high}` wrapped under the
   settings key, carrying a key it chose; the administrator's device trusts it,
   `sealing_key(X)` picks it, and what that administrator writes in X is
   sealed under a key the server knows (a scope row likewise pulls children
   under it). This contradicts decisions/a_server_sealed_key_never_seals_anything.md;
   its test covers only a member. Fix: a key the server holds is never trusted
   for wrapping, on any device; with 3, a wrap is trusted only under its own
   block's scope key.
3. **Sealed objects are not bound to their block.** `Keyring::open`
   (be-block-store vault.rs:140) opens with whatever key id the bytes name;
   `Peer::open_metadata` (peer.rs:1515), `CommitStore::get`, `Peer::unseal`
   (relays, peer.rs:2824) and `open_notification` read through it, and nothing
   checks the key is one of the block's, nor trusted; `Commit` names no block.
   Attack, by the server: serve as block B's head or metadata a commit or name
   sealed under another key the victim holds (the settings data key, which
   every member and the server hold, or block A's key), or deliver Alice's
   relay of A's session as B's; the device shows it as B, and on saving or
   owning B's session writes it into B under B's real key. In a personal
   organization, any viewer of A colluding with the server forges B to anyone
   who also reads A. Chat alone is safe (its signatures bind the block). Fix:
   open an object only with a key of the block being read; better, seal with
   the block id and the object's role as associated data.

### Medium

4. **A contact's key can be rolled back.** The device reads its contacts from
   the server each time (`contacts::contacts`, contacts.rs:173): each row's
   own-sealed card proves only that this account sealed it once, and
   `KeyBook::set_contacts` replaces the list whole. Attack, by the server with
   whoever holds contact C's old phrase (what a new phrase exists to stop):
   serve the old own-sealed card for C with no `changes`; the device trusts
   C's old key again, seals scope keys to it and takes keys sealed with it as
   trusted. Signing keys are safe (`Signers` never moves back). Fix: pin each
   contact's newest identity key locally, as `Signers` does, and refuse an
   older card. With it, give `seal_own` a context per kind (simplicity 2).
5. **A contact's name comes from the server.** `contacts::contacts` lists
   `row.display_name` and `row.email` (contacts.rs:196), not the verified
   card's. Attack, by the server, against someone with two verified contacts
   Bob and Mallory: label Mallory's row "Bob"; sharing with "Bob" seals the key
   to Mallory. Signed grants (plans/signed_access.md) don't help: the user
   picks the wrong account. Fix: show the card's name and email.
6. **An owner-only block's data keys can be escrowed.** `escrow_wrap_under`
   (keys.rs:329) refuses only version-8 key ids, which are owner-only scope
   keys; the data keys under such a scope have ordinary ids, and key work
   escrows any trusted key the server lists as unescrowed (rotation.rs:59).
   Attack, by the server: list an owner-only block's data key as unescrowed;
   the owner's device escrows it and administrators read the owner-only
   settings, breaking decisions/owner_only_settings_are_never_escrowed.md. The
   `unsealed` loop likewise seals the owner-only scope key to a listed contact
   (that part overlaps signed_access). Fix: refuse anything inside an
   owner-only scope, not just its scope key.
7. **A deletion is checked against whoever signed it, not the message's
   author.** Chat offers Delete only on one's own messages, but
   `forged_deletions` (be-app-chat ui.rs:1350) verifies `deletion.author`, and
   `Deletion::signed_bytes` binds neither the message's author nor its hash.
   Attack, by a member with Edit: delete Alice's message with a deletion signed
   as themselves; it verifies, so no "forged" row shows, and the gap row is
   suppressed because a deletion exists: the message vanishes without a mark,
   where a removal outright would show. Fix: sign the deleted message's `Prior`
   into the deletion and check it against the message's author.
8. **A device code is approved with a 30-day cookie and mints a token that
   never expires.** `/auth/approve` (be-server portal.rs:327) asks no fresh
   proof, and `issue_token` (store.rs:1036) makes an `app` session with no
   expiry that `resolve_token` never checks. Attack: whoever holds a victim's
   browser cookie (a shared computer), or phishes a signed-in victim to
   `/auth?code=ATTACKER`, gets an app token for the account that outlives the
   browser session and its sign-out. Fix: approving needs a fresh proof, or the
   token takes the browser session's expiry until the device is confirmed.
9. **Recovery keeps what an intruder added.** `open_browser_session(.., recover)`
   (store/codes.rs:263) ends sessions and email changes but keeps passkeys,
   providers and agents. Attack: an intruder who held the account briefly adds
   a passkey or an agent (`CreateAgent` needs only an app session); after the
   owner recovers, the intruder signs straight back in, and the agent's token
   still works. Fix: recovery lists every identity and agent for review and
   drops those added since the last proof the owner trusts. Agents overlap
   signed_access.
10. **The escrow chain can be forked by a demoted administrator with the
    server.** `verify_escrow` (keys.rs:134) accepts a newer epoch signed by the
    last key the device verified, which the demoted administrator still holds;
    the server serves their epoch to members pinned at the old one and hides
    the real one, so `escrow_warning` never fires, and those members escrow to
    the demoted administrator for good. The server doesn't check chain
    signatures (be-server keys.rs:1088). Overlaps signed_access step 3: bind
    escrow epochs to administrator records in the organization log.

### Low

11. **Key changes race like retirements.** `changed_key` (contacts.rs:217)
    takes the first change the server lists that opens; a holder of the old
    phrase sends their own, and a contact who applies it first follows them.
    The guide documents the race for logs only. Document it with them.
12. **No low-order check on X25519 keys.** `diffie_hellman` results are never
    checked `was_contributory` (be-block-keys lib.rs:240, 284, 354), and
    public keys from cards and key changes are taken unchecked. A contact (or an
    administrator choosing an escrow public) who publishes a low-order point
    makes seals to and from it forgeable and readable by anyone. Fix: refuse a
    non-contributory result.
13. **A wrapped key does not name its own id.** `CarriedKey` is key, block and
    root; the server can relabel a rewrap of an old data key as a new epoch of
    the same block (within the accepted "server decides the current key", but
    it makes a device that rotated write under the old key). Fix: carry the
    key id, kind and epoch.
14. **Backups are not bound to their names.** A backup's seal has no
    associated data (be-server backup.rs:270) and restore picks the newest name
    (backup.rs:419): whoever can write the backup storage can roll the server
    back to a database before revocations, reviving revoked sessions. Fix: the
    snapshot's time as associated data, shown on restore.
15. **An email change is confirmed by its link alone** (store/portal.rs:222):
    an attacker's account requests a change to a victim's unregistered address
    and the victim's click attaches it to the attacker's account.
16. **Smaller server items**: `Transfer-Encoding` is ignored rather than
    refused (http.rs:1021); a begun provider sign-in's state is spent before its
    browser is checked (providers.rs:334); pending sign-ins evict per /64,
    so a /48 pushes out everyone's.
17. **A server-triggered new phrase**: told there is no recovery key,
    be-host-stack's `KeyState::loaded` (keys.rs:56) sets up and saves a new
    identity, replacing the stored secret (denial of service, and confusing).

## Simpler: two ways to one thing

1. **Seal to oneself** two ways: `seal_own` and `seal_for` to one's own key.
   Keep one, `seal_own(context, plain)`.
2. **`seal_own` has no context**: cards, link codes, invitation codes, pins
   and own seals share one key and only some check what they open. One typed
   context per kind (closes 4's replay across kinds too).
3. **`KeyBook::contact` answers two questions** (1). Split it.
4. **Three symmetric seal formats**: the vault's (`be3K`, key id, derived
   nonce), be-block-keys' `seal_with` (random nonce, no header) and backups'.
   And two seals to a public key: `seal_for` and `seal_anonymous`, the latter
   used only for escrow (an `EscrowPublic` type would keep it there).
5. **Deterministic nonces where nothing deduplicates**: relays, notifications,
   metadata and journal entries use the content-addressing seal, which shows
   the server when two are equal (a presence or toggle repeated) and gains
   nothing. decisions/deterministic_seal_nonce.md argues it for content-addressed
   objects only. Seal those with a random nonce.
6. **Two key derivations**: HKDF in be-block-keys, raw SHA-256 in
   `ServerKey::derived` (be-server organization.rs:84). And five generators of
   a random secret in be-server (OsRng hex, two UUIDv4s, one UUIDv4, a UUID
   slice, `rand::rng`): one `new_secret()`.
7. **Four encodings of signed bytes** (postcard after a domain, fixed-width
   concatenation, chat's builder, the host's framed app id): each unambiguous,
   but one helper would serve all.
8. **The fresh-proof query and its update** are written three times in
   be-server (codes.rs, store/passkeys.rs, store/providers.rs).
9. **Escrow keys sign with a raw `[u8; 32]`** and callers add the domain,
   while the identity key's domains live in be-block-keys. Put the escrow
   chain's domain in be-block-keys too.
10. **Public `InviteCode::seal`/`open` take any context**; every caller goes
    through `seal_grant`, `seal_escrow` or `ContactCard::seal`. Make them
    private.

## Primitives and parameters

- **Libraries** are well reviewed and current: chacha20poly1305 0.10, x25519
  and ed25519-dalek 2 on curve25519-dalek 4.1.3 (past the timing fix), hkdf
  0.12, sha2 0.10, spake2 0.4, bip39 2; webauthn-rs and openidconnect on the
  server; ring and p256 for web push.
- **Randomness**: OsRng (or a CSPRNG) for every secret; codes are unbiased.
  Entropy: tokens 256 bits, links 244, form tokens 122, invite codes 200 on
  devices and 100 on the server, the device code 48, mailed codes 39 (five
  tries, ten minutes), pairing 40 (SPAKE2, one online guess). Fine.
- **The vault's nonce** is SHA-256(key ‖ plaintext) cut to 96 bits, with no
  domain and the key itself as input, and no associated data: safe for
  content-addressed chunks (collisions near 2^48 seals per key, keys are per
  block), but not a real SIV (the nonce isn't checked on open). With 3: derive
  a nonce key by HKDF, use XChaCha20-Poly1305 (192-bit nonce) or AES-GCM-SIV,
  and put the header and block id in associated data.
- **Domain separation** holds: every HKDF info and signing domain is distinct
  and none prefixes another (`be3.access.v1` and `be3.access.retirement.v1`
  differ at the eighth byte after the dot); variable-length parts come last.
  be-server's `hash_token` takes no prefix for sessions, links, attempts and
  OIDC state; harmless at those entropies, but prefix them.
- **Signatures**: `verify_strict` everywhere; entries are re-encoded before
  hashing, so postcard's form doesn't matter.
- **Constant time**: secrets are looked up by hash; AEAD tags inside the
  libraries. Plain `==` on in-memory secrets (`InviteCode`, `RecoveryPhrase`,
  be-server's device secret and form owner) has no remote oracle.
- **Zeroization**: `StaticSecret` and `SigningKey` wipe themselves; the phrase
  entropy, `EscrowSecret`, `InviteCode`, `PairingKey`, derived keys,
  `CarriedKey.key` and opened identity secrets don't. Add `zeroize`.
- **Secrets in `Debug`**: `CarriedKey`, `GrantOffer`, `host_state::Write::SetIdentity`,
  `SavedAccount` (token), `RecoveryView` (phrase) and `Invite.link` derive it.
  Nothing logs them today; redact them so nothing can.
- **At rest**: the identity secret sits in plain text beside the vault and the
  token (`identity_secrets`, be-host-base kept.rs), with no OS keystore, so a
  device thief has the account. The server account's secret is plain in its
  database, so a backup with its key opens the settings keys sealed to the
  server, the S3 credentials and every push `auth` secret. Both by design;
  neither is written down (guides/blocks/keys.md, guides/hosting.md).
- **Web push** follows RFC 8291 and 8188 exactly.

## Where the guides are wrong

- guides/blocks/keys.md, Keys: "sealed bytes name their key ... covers wrapped
  keys": only wraps under a key use `be3K`; seals to accounts, escrow, own,
  invitation and pairing seals use random nonces and no header.
- keys.md, Trust: "a key the server made up never protects anything new"
  (2); "a contact's key changes only by a change sealed with the verified key"
  (4); Escrow: owner-only "sealed to its author alone" (6); "What the server
  can and cannot do" omits 1, 3 and 5.
- guides/blocks/client.md: notifications "sealed under the block's data key";
  any held key opens one (3).
- guides/chat.md: "a signed one leaves none" holds for the message's author
  only (7).
- guides/blocks/server.md: a form token "bound to the browser's session" is
  shared by every signed-out browser (the Origin check is what stops login
  CSRF there); cookies over plain HTTP are shadowed by any localhost service
  (ports don't scope cookies); app sessions never expire.
- decisions/workspace_keys_without_device_key_pairs.md says the recovery key
  seals keys "for invited members"; an invitation hands over no key.
- decisions/accounts_sign_in_with_identities_the_operator_configures.md:
  recovery "ends every other session" but keeps added identities (9).

## Checked and sound

Sender-authenticated seals bind both keys, both accounts and the key id;
a server's seal is accepted only, untrusted, for the settings key. Pairing:
SPAKE2 bound to the account, the seal confirms the key, one wrong guess ends
it. Invitations and links: role-split cards, a lookup id the code can't be
recovered from, grants and escrow pins bound to their ids. Access logs:
`check_log` checks kind, log id, chain, signer and retirements. Owner-only
scope key ids are bound into every seal. Rotation gives revoked members no
future keys, given an honest current epoch. The `sign` and `verify` host calls
keep the key in the worker and frame the app's id. On the server: WebAuthn
(single-use challenges, RP id from the configured origin, user handle checked,
counter checked in the write), mailed codes, login links, invite codes, OIDC
(PKCE, state bound to browser and provider, nonce, issuer, audience,
`max_age=0`), Origin checks, hashed tokens, revocation closing sockets, and
test-only fakes behind `cfg(test)`.

## Order

1, 2 and 3 first, together: they share the fix of tying a key to its block and
to who may hold it. Then 4 and 5 (contacts), 6, 7, 8, 9. 10 goes with
signed_access step 3. The low items and the simplifications can follow in any
order, except the vault's nonce and associated data (Primitives), which
belongs with 3.
