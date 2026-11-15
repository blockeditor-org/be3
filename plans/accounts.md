# Accounts: sign-in without passwords

Depends on: none.

Today a remote account is an email and a password. The `/auth` portal verifies
the email, an operator invite code gates registration, and be-server hashes and
checks passwords behind a bounded queue with lockouts, slowdowns and capped mail
links (guides/blocks/server.md, "Accounts"). The portal also changes an email
(`email_change_link`), resets a password and lists browser sessions; changing
a password ends every session of the account. Mail goes through
`mail::Delivery`, whose only implementation, `LogDelivery`, writes every link to
the server log, the hosted server's included (guides/hosting.md, "Accounts and
the portal"). Agents and local profiles already have no password: a token is
their only credential. This plan replaces the password with sign-in methods that
are cheaper to run, without requiring any one of them on every device or server.

It covers accounts on a server. A local account (plans/local_accounts.md) has no
server, so nothing here applies to it; until that plan deletes the embedded
server, a local profile keeps its token and has no identity.

## Why

- **Content keys are not the account.** Workspace keys live on devices and are
  sealed to a recovery phrase (decisions/workspace_keys_without_device_key_pairs.md),
  and the account's signing key derives from the same phrase
  (decisions/an_ed25519_signing_key_beside_the_identity_key.md). An account
  decides who may connect, who is a member and who is charged for storage.
  Taking one over gets metadata and memberships, never plaintext and never a
  signature, and a new device still needs a paired device or the phrase. So an
  email-based factor is strong enough as the baseline.
- **Passwords are the costliest part of the server's account code**: the
  hashing pool and its per-address queue, (email, address) and per-address
  lockouts, the per-email slowdown, remembered addresses, reset caps. Credential
  stuffing and reuse remain a risk however well it is done.
- **Self-hosting rules out one central login provider.** Every server is its
  own account domain, so any provider must be something the operator chooses and
  configures.
- **Passkeys are not available everywhere**, and their UI is still uneven, so
  they cannot be the only way in.

## Target

An account has an opaque id and a set of **linked identities**. Proving any one
signs in. One mechanism, three kinds:

1. **Verified email, with a one-time code and a link that fills it in.** The
   baseline: it works on any device with a browser, stores no secret and
   doubles as recovery (it can add or replace any other identity).
2. **Passkey (WebAuthn).** An upgrade offered after sign-in and used when
   present; never required.
3. **OIDC identity `(issuer, sub)`.** Sign in with Google, Apple, Microsoft or an
   organization's own provider. Generic OIDC is the one implementation; Google
   is a preset of it, and Apple a later one (plans/deferred/apple_sign_in.md).
   A server with no providers configured shows no such buttons.

An account needs no email: one whose identities are only providers or passkeys
is allowed, and those are its recovery. An account always keeps at least one
identity; the last can't be removed. Where an email is shown for an account
today, the contact card and "Connect with NAME (EMAIL)?" among them, an account
with no verified email shows the email its provider verified if the operator
marks that provider authoritative for email (below), else only its name.

Sign-in stays in the browser portal (`begin-browser-login`,
`poll-browser-login`, and the portal's `/auth/code`, approve and deny), so the
credential only has to work in a browser. be-host already always shows the URL
and code beside the button that opens a browser
(`browser_authorization_always_offers_the_link_and_code`), so a browser that
fails to open, or opens the wrong one, still has a way through: the person opens
the URL on another device, signs in and approves there, and the first device's
poll completes. A device with no browser (the PineNote, a login session with no
browser program) relies on that alone. Every method here must work through that
URL, so none may need the browser to be on the device being signed in. No
client needs OAuth or WebAuthn code of its own. The approve page keeps its
warning that approving signs in another device, since that approval can be
phished whatever signed in to the portal; it may also show the requester's
rough location.

Agent accounts keep their tokens (guides/tracker.md), and header
authentication through a gateway (`Be3-Gateway-Authorization`) stays as it is.

## Rules every method follows

- **Email codes.** Eight characters from an alphabet without look-alikes,
  valid ten minutes, dead after five wrong tries. A code belongs to the one pending attempt that asked
  for it (an attempt id held in the requesting browser's cookie), never to the
  email alone, and a new request for the same attempt ends the older code. The
  mail carries the code and a link that fills the code in; it completes sign-in only in
  the browser that began the attempt. Opened anywhere else it shows the code and
  signs nothing in, so nobody can sign a victim's browser in to an attacker's
  account (login CSRF).
- **Mail limits strangers can't spend.** Today resets are capped at three per
  account, which a stranger can use up to lock the owner out of recovery.
  Instead: caps per address (an IPv6 address by its /64, as now), and a large
  per-account budget that refills quickly, counting only requests from addresses
  that never completed a sign-in for that account (a small, bounded remembered
  addresses table, as passwords keep today). A request for an email with no
  account does the same work and sends nothing, so total mail is at most the
  number of accounts times the per-account rate, and no global cap is needed
  (one reachable by anonymous traffic would lock everyone out). Registration
  links stay capped per invite code, and the global cap stays only on
  registration and email-change links, as now.
- **Registration enumeration.** Registering with an email that has an account
  is refused today (`unregistered` in `store/portal.rs`), which tells a stranger
  the email is registered. Behind an operator invite code that leak is small.
  Keep the refusal while registration needs a code, and record it in the guide;
  mailing "you already have an account" instead is the alternative if
  registration opens.
- **Fresh proof for account changes.** Changing the email, adding or removing an
  identity, and ending other sessions need a proof from the last few minutes
  with whichever identity the account has: a code sent to its email, a passkey,
  or a provider sign-in. An email change (the existing `email_change_link` flow,
  now behind fresh proof) also mails the old address. Every identity added or
  removed mails a notice to the account's email, if it has one. Signing
  in through an emailed code after "I can't sign in" (recovery) ends every
  other session by default, as a password change does today.
- **Mail goes to the log.** Real delivery is deferred
  (plans/deferred/mail_delivery.md); `LogDelivery` stays, so the server's
  journal holds every live sign-in code and link, and whoever reads it can sign
  in to any account with an email. That is only a little worse than today:
  the log already holds every verification and reset link, and the operator
  now reads it at each device's sign-in rather than only at account creation.
  On the hosted server the operator login link (step 3) is the documented way
  in.
- **Operator links.** `be-server login-link` (step 3) makes a single-use link
  valid fifteen minutes, keeps only its hash, and builds it from
  `--auth-origin`.

## Steps

1. **Identities and emailed codes, beside passwords.** Add identities: an
   account id, and identities of the kinds above; email stays a verified
   attribute for contact and recovery, and an account may have none. Add
   sign-in and registration by emailed code through the portal, following the rules above, while password sign-in
   still works. The portal's settings page lists identities, to add and remove
   (never the last); its sessions page stays and lists app sessions too. Tests
   read codes from a capturing `Delivery` (as `tests.rs` already has one), or
   from the portal's JSON `Page` that a server without a `Portal` answers.
   Done: tests where a code signs in only the browser that asked for it; a link
   opened in another browser shows the code and signs nothing in; a sixth wrong
   try, an expired code and a superseded code are each refused; requests from a
   stranger's addresses never stop the owner getting a code from an address
   they signed in from; an email with no account sends nothing; an email change
   without fresh proof is refused and with it mails the old address.
2. **Passwords go**, once step 1 signs in by code: remove the password
   column, `store/passwords.rs`, the hashing queue, the lockouts and slowdown,
   the reset flow and the password settings page. `address_key` moves from
   `passwords.rs` to `address.rs`, beside its `IpAddr` form, and the remembered
   addresses table stays for the mail limits. `--allow-login` then gates
   signing in by any identity, and `--allow-registration` making an account
   by any identity, which still needs an operator invite code. No
   migration, since no format is frozen (guides/frozen_formats.md). Done: verify
   with the password code gone; the portal tests sign in only through codes.
3. **Operator login link.** For any server whose mail goes to the log (every
   server until plans/deferred/mail_delivery.md lands),
   `be-server login-link --data-dir DATA (--email EMAIL | --account ID)`
   (beside `invite-code create`, run against the data directory while the
   server runs) prints a link as the rules above say, in place of finding the
   code in the log. Given an email with no account, the link registers that
   account (no invite code: only the operator runs the command), so a new
   server's first account needs nothing from the log either. Done: tests where
   a link signs in once and a second use and an expired one are refused, and
   one where a link for a new email makes its account.
4. **OIDC providers.** Operator config lists providers: issuer, client id and
   secret, and whether the provider is authoritative for email. Use the
   `openidconnect` crate. Rules:
   - Authorization code with PKCE, `state` and `nonce`, and a full ID token
     check: signature against the provider's JWKS, `aud` and `azp`, expiry,
     nonce. The redirect is fixed at
     `{auth-origin}/auth/oidc/{provider}/callback`.
   - Key on `(iss, sub)`, with `iss` exactly as discovery gives it
     (Microsoft's multi-tenant issuer names the tenant), never on email: emails
     change, and Apple's may be a relay address.
   - Trust `email_verified` only from providers the operator marks as
     authoritative for email (Google). Never store or reserve an email the
     provider has not verified. An account made through a provider needs no
     email of its own.
   - When a provider's verified email (from an authoritative provider) matches
     an existing account's email, linking asks once for a code sent to that
     email first; it never links automatically. Otherwise a provider sign-in
     with no linked identity makes a new account, and only a signed-in user
     adds a provider to an existing one, with fresh proof.
   - The portal's CSP has `form-action 'self'`, so sign-in with a provider
     starts as a GET link to `/auth/oidc/{provider}/start`, which redirects.
   - Registering through a provider needs the operator invite code while
     registration needs one.
   - Nudge anyone whose only identity is a provider to link a second one, so
     losing a Google account does not lock them out.
   - GitHub is OAuth2, not OIDC; skip it until asked.
   Done: tests against a fake OIDC provider: a token with the wrong `aud`, a
   bad signature, an old nonce or a reused `state` is refused; an unverified
   email links nothing; a provider email matching an account links only after
   the emailed code.
5. **Passkeys.** Register and use WebAuthn credentials in the portal with
   `webauthn-rs` on the server and `web-sys`'s credentials API from the portal's
   wasm, which the CSP already allows (`script-src 'self'
   'wasm-unsafe-eval'`, `connect-src 'self'`). The RP ID is the
   `--auth-origin` host. Passkeys are discoverable, so signing in with one
   needs no email first. Allow several per account and list them for removal.
   Push them only once the browser UI is good enough. Changing `--auth-origin`
   orphans every passkey and every provider's registered redirect URI; say so
   in guides/hosting.md. Done: tests with a software authenticator: register,
   sign in without an email, remove, and a passkey for another RP ID refused.
6. **Invitations stop depending on email.** Today an invitation is already a
   link the inviter sends themselves (`{server}/#invite/{CODE}`, a link of kind
   `invitation`; guides/blocks/keys.md, "Inviting"), and the server mails none.
   But it is bound to an email: only the account with that email reads or
   accepts it, and a new one for the same email replaces the outstanding one.
   - Drop the email. An invitation names an organization and joins as an
     editor, never as an administrator: an administrator is sealed the escrow
     key, so a leaked administrator invitation would hand over the
     organization. Making someone an administrator is `set-role` on a member
     who is already the administrator's verified contact. Whoever opens the link
     with an account on the server may accept it; for an editor, the list
     showing who accepted, and `remove-member`, are enough, with no confirming
     step.
   - A link is then a bearer secret: single use, expiring after seven days by
     default, and the id reaches the invitee only in the link,
     as now. At most a bounded number outstanding per organization, in place of
     one per email.
   - Reading the invitation before signing in: `get-link` needs a signed-in
     account today. Add an unauthenticated read by link id that answers the
     organization name (in the clear on the server already) and the sealed
     card, rate-limited per address; the host opens the card with the code and
     shows who invited them and to what. The alternative, showing the card only
     after sign-in, is simpler but sends people into registration not knowing
     who invited them.
   - The link works for someone with no account. The server never sees what
     follows `#`, so the host (the web build, or be-host) keeps the code, sends
     them through the portal to register or sign in with any identity above, and
     returns to the invitation with the code still held. An operator invite code
     still applies to registering; the portal asks for one when the server
     requires it. plans/share_links.md needs the same round trip for a share
     link's secret; whichever lands second uses the first one's mechanism.
   - Opening it signed in as the wrong account (one already a member, say) says
     whose invite it is and offers to sign in as another, not a bare error.
   - **Kept rows.** Today `finish_links` and `delete-link` delete a link's row,
     and links never expire. The invite list needs rows kept after accepting,
     expiring and revoking: new, with a bound per organization and a retention
     period after which they go.
   - **The invite list** (new, in the organization screen,
     `crates/be-host/src/ui/organization.rs`, with the Invite member dialog in
     `crates/be-host/src/ui/dialogs.rs`): creating an invite ends on the link,
     with copy, the platform share sheet and a QR code, and says plainly that
     nothing was sent (today the dialog asks for an email and shows the link
     once in a field). Each invite shows its state: waiting, opened (the server
     answered a read for it, which tells the inviter the link arrived), accepted,
     expired or revoked, who accepted, and when. Revoke, and copy the same link
     again, from the list: the inviter's device keeps the code sealed for itself
     (`RecoverySecret::seal_own`). An expired invite offers a fresh link in one
     step.
   - Accepting makes the two accounts contacts, as now.
   Done: tests where an invitation joins an account with any email as an
   editor; one opened a second time, an expired one and a revoked one are
   refused; the read before sign-in answers the organization name and sealed
   card only; the list shows opened and accepted; a painting of the invite list
   and of the link with its QR code.
7. **Docs.** Rewrite "Accounts" and "Running it" in guides/blocks/server.md,
   "Accounts and the portal" in guides/hosting.md (providers, `login-link` as
   the way in, codes in the log, and that changing `--auth-origin` orphans passkeys and
   redirect URIs), and the invitation lines in guides/blocks/keys.md
   ("Inviting", "Contacts"); add the decision records below.

Registration admission (operator invite codes) stays a separate policy from the
credential kind and from member invitations. It is temporary, for while the
server is not safe enough to let anyone make an account, and for self-hosters
who don't want strangers signing up to their node, so it gets no UX work beyond
asking for the code when the server wants one.

## Other plans

- **Share links** (plans/share_links.md, in flight) let an account on the server
  open a block's link; the opener signs in or registers through this plan's
  portal like an invitee, keeping the secret across it the same way. Opening a
  link with no account at all is deferred
  (plans/deferred/anonymous_share_links.md); its question of what a server asks
  of an account made without an email becomes "with no identity" once this
  lands, and is that plan's to answer.
- **Signed access** (plans/signed_access.md, in flight) puts the organization
  root's hash in the invitation, sealed with the code. Nothing here changes
  that; it strengthens the argument above, since records are signed with the
  phrase's key and an account takeover cannot sign them.
- **Local accounts** (plans/local_accounts.md) later gain a server "as pushing
  to a new remote"; that registers an account here and is part of that plan.

## Decisions to record

- Passwords are removed. They cost the hashing pool, lockouts and slowdowns,
  and carry credential stuffing and reuse; an emailed code is enough because an
  account holds no content keys. Rejected: keeping them as a fallback, which
  keeps all of that machinery for no gain, since email recovery already
  overrides any other credential and approving a device code can be phished
  whatever signed in to the portal. Sign-in codes go to the server log until
  mail delivery lands, which is only a little worse than today's
  verification links there. A device with no browser signs in by
  opening the URL on another device, so none needs one.
- Login through a central provider only (rejected): breaks self-hosting, leaks
  metadata and makes us depend on someone else's recovery.
- An account that is only a bare keypair (rejected): fits end-to-end encryption
  but key loss has no safety net beyond the recovery phrase and it is hard to
  explain; passkeys give most of the benefit with platform sync.
- Invitations are links the inviter sends, never mail the server sends or
  anything bound to an email, and they join as editors only.
- Providers are configured by the operator, per server; accounts do not federate
  across servers.
- An account needs no email; its providers and passkeys are its recovery. A
  provider's email links to an existing account only after a code sent to it.

