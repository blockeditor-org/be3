# Accounts: sign-in without passwords

Depends on: none.

Today a remote account is an email and a password. The `/auth` portal verifies
the email, an operator invite code gates registration, and be-server hashes and
checks passwords behind a bounded queue with lockouts, slowdowns and capped mail
links (guides/blocks/server.md, "Accounts"). Email delivery is not connected
yet; verification messages go to the server log. This plan replaces the
password with sign-in methods that are cheaper to run and safer to hold, without
requiring any one of them on every device or server.

## Why

- **Content keys are not the account.** Workspace keys live on devices and are
  sealed to a recovery phrase (decisions/workspace_keys_without_device_key_pairs.md).
  An account decides who may connect, who is a member and who is charged for
  storage. Taking one over gets metadata and memberships, never plaintext, and a
  new device still needs a paired device or the phrase. So an email-based factor
  is strong enough as the baseline.
- **Passwords are the costliest part of the server's account code**: hashing
  pool, per-address and per-email lockouts, slowdowns, remembered addresses,
  reset caps. Credential stuffing and reuse remain a risk however well it is
  done.
- **Self-hosting rules out one central login provider.** Every server is its
  own account domain, so any provider must be something the operator chooses and
  configures.
- **Passkeys are not available everywhere**, and their UI is still uneven, so
  they cannot be the only way in.

## Target

An account has an opaque id and a set of **linked identities**. Proving any one
signs in. One mechanism, three kinds:

1. **Verified email, with a one-time code or link.** The baseline: it works on
   any device with a browser, stores no secret and doubles as recovery (it can
   add or replace any other identity).
2. **Passkey (WebAuthn).** An upgrade offered after sign-in and used when
   present; never required. Phishing-resistant and no hashing queue.
3. **OIDC identity `(issuer, sub)`.** Sign in with Google, Apple, Microsoft or an
   organization's own provider. Generic OIDC is the one implementation; Google
   and Apple are presets of it. A server with no providers configured shows no
   such buttons.

Sign-in stays in the browser portal (`begin-browser-login`,
`poll-browser-login`), so the credential only has to work in a browser, and a
device with no usable browser or passkey UI (the login session, the PineNote)
approves from another device or with an emailed code. No client needs OAuth or
WebAuthn code of its own.

Agent accounts keep their tokens (guides/tracker.md); they are already
passwordless.

## Steps

1. **Identities table.** Split credentials from accounts: an account id, and
   identities of the kinds above. Email stays a verified attribute used for
   contact and recovery. Remove the password column and
   `store/passwords.rs`, the hashing queue, the (email, address) lockouts and
   the reset flow; no migration, since no format is frozen
   (guides/frozen_formats.md). Keep per-email and per-address caps on mail.
2. **Emailed code sign-in and registration.** One-time code (or link) through
   the portal, replacing password registration and sign-in. Connect real email
   delivery here; until then keep printing to the log.
3. **Operator login link.** For a self-hosted server without SMTP, a
   `be-server login-link` command (beside `invite-code create`) prints a
   one-time link for an account, replacing the "read it from the log"
   workaround.
4. **OIDC providers.** Operator config lists providers (issuer, client id,
   secret). Rules:
   - Key on `sub`, never on email: emails change, and Apple's may be a relay
     address.
   - Link a provider identity to an existing account automatically only when
     the provider says `email_verified` and the account's email is verified;
     otherwise only a signed-in user adds it explicitly. Unverified-email
     linking is an account takeover.
   - Nudge anyone whose only identity is a provider to link a second one, so
     losing a Google account does not lock them out.
   - Apple needs a paid developer account, a rotating signed client secret and
     stores the name only on first sign-in, so keep it then. This is the
     operator's cost, which is why presets are optional.
   - GitHub is OAuth2, not OIDC; skip it until asked.
5. **Passkeys.** Register and use WebAuthn credentials in the portal, scoped to
   `--auth-origin`; allow several per account and show them for removal. Push
   them only once the browser UI is good enough.
6. **Invitations stop depending on email.** An `Invite` is bound to an email
   today (only an account with that email can read or accept it). That ties
   email back into authorization and sits badly with the rule that every sharing
   relationship starts with an out-of-band contact link. Make an invite
   redeemable by the holder of the link, or by a contact, instead.
7. **Docs.** Rewrite "Accounts" in guides/blocks/server.md, the running section
   (guides/blocks/server.md, "Running it") and guides/hosting.md; add the
   decision records below.

Registration admission (operator invite codes) stays a separate policy from the
credential kind.

## Decisions to record

- Passwords are removed. Rejected: keeping them as a fallback, because it keeps
  the whole hashing and lockout machinery and weakens the passkey guarantee.
  Revisit only if a client appears that cannot use the browser portal; then make
  a password an explicit opt-in identity.
- Login through a central provider only (rejected): breaks self-hosting, leaks
  metadata and makes us depend on someone else's recovery.
- An account that is only a bare keypair (rejected): fits end-to-end encryption
  but key loss has no safety net beyond the recovery phrase and it is hard to
  explain; passkeys give most of the benefit with platform sync.
- Providers are configured by the operator, per server; accounts do not federate
  across servers.

## Open questions

- Is there any client that truly cannot open the browser portal? If none, passwords
  stay deleted.
- Emailed code or link? A link is easier on one device; a code works when the
  mail arrives on another. Probably offer a code that the link also fills in.
- Does the hosted server want Apple at launch, given the developer-account cost?
