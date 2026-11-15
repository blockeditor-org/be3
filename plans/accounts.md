# Accounts: sign-in without passwords

Depends on: none.

Today a remote account is an email and a password. The `/auth` portal verifies
the email, an operator invite code gates registration, and be-server hashes and
checks passwords behind a bounded queue with lockouts, slowdowns and capped mail
links (guides/blocks/server.md, "Accounts"). The portal also changes an email,
resets a password and lists browser sessions. Mail goes through
`mail::Delivery`, whose only implementation, `LogDelivery`, writes every link to
the server log (guides/hosting.md, "Accounts and the portal"). Agents and local
profiles already have no password: a token is their only credential. This plan
replaces the password with sign-in methods that are cheaper to run and safer to
hold, without requiring any one of them on every device or server.

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
`poll-browser-login`, and the portal's `/auth/code`, approve and deny), so the
credential only has to work in a browser. be-host already always shows the URL
and code beside the button that opens a browser
(`browser_authorization_always_offers_the_link_and_code`), so a browser that
fails to open, or opens the wrong one, still has a way through: the person opens
the URL on another device, signs in and approves there, and the first device's
poll completes. A device with no browser (the PineNote, a login session with no
browser program) relies on that alone. Every method here must work through that
URL, so none may need the browser to be on the device being signed in. No
client needs OAuth or WebAuthn code of its own.

Agent accounts keep their tokens (guides/tracker.md), and header
authentication through a gateway (`Be3-Gateway-Authorization`) stays as it is.

## Steps

1. **Identities table.** Split credentials from accounts: an account id, and
   identities of the kinds above. Email stays a verified attribute used for
   contact and recovery. Remove the password column, `store/passwords.rs`, the
   hashing queue, the lockouts and slowdown, remembered addresses, the reset
   flow and the portal's password settings page; no migration, since no format
   is frozen (guides/frozen_formats.md). The portal's settings page lists the
   account's identities, to add and remove (never the last one); its sessions
   page stays.
2. **Emailed code sign-in and registration.** A one-time code (or link) through
   the portal replaces password registration, sign-in and reset. Mail stays
   capped per email and purpose, per address and per invite code, as now; a
   sign-in mail is capped per account as resets are today, and one for an email
   with no account does the same work and sends nothing, so the portal does not
   say which emails have accounts. A code takes a few wrong guesses and then
   dies, which replaces the lockouts. Connect real email delivery (a
   `mail::Delivery` that sends) here; until then `LogDelivery` keeps printing
   to the log.
3. **Operator login link.** For a self-hosted server without mail delivery, a
   `be-server login-link` command (beside `invite-code create`, run against the
   data directory as it is) prints a one-time sign-in link for an account,
   replacing "read it from the log".
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
   `--auth-origin`; allow several per account and list them for removal. Push
   them only once the browser UI is good enough.
6. **Invitations stop depending on email.** Today an invitation is already a
   link the inviter sends themselves (`{server}/#invite/{CODE}`, a link of kind
   `invitation`; guides/blocks/keys.md, "Inviting"), and the server mails none.
   But it is bound to an email: only the account with that email reads or
   accepts it, and a new one for the same email replaces the outstanding one.
   Drop the email: an invitation names an organization and a role, and whoever
   opens the link with an account on the server may accept it. A link is then
   a bearer secret, so it is single use and expires, and the id reaches the
   invitee only in the link, as now. Keep at most a bounded number outstanding
   per organization in place of one per email. The flow has to be good, because
   the failure to avoid is an invitee who can't find the invite while the
   inviter can't tell why:
   - Creating an invite ends on the link, with copy, the platform share sheet
     and a QR code, and says plainly that nothing was sent. Today the Invite
     member dialog asks for an email and shows the link once in a field.
   - The invite list (`list-links`) shows each one's state: waiting, opened,
     accepted, expired or revoked, who accepted, and when. "Opened" (the server
     answered `get-link` for it) is what tells the inviter the link arrived and
     the invitee is mid-way, so a quiet invite is not a mystery.
   - Revoke, and copy the same link again, from the list: the inviter's device
     keeps the code sealed for itself (`RecoverySecret::seal_own`). An expired
     invite offers a fresh link in one step.
   - The link works for someone with no account. The server never sees what
     follows `#`, so the host (the web build, or be-host) holds the code, opens
     the card and shows who invited them and to what, then sends them through
     the portal to register or sign in with any identity above and returns to
     the invitation with the code still held. An operator invite code still
     applies to registering, invite link or not; the portal asks for one when
     the server requires it.
   - Opening it signed in as the wrong account (one already a member, say) says
     whose invite it is and offers to sign in as another, not a bare error.
   - Accepting makes the two accounts contacts, as now.
7. **Docs.** Rewrite "Accounts" and "Running it" in guides/blocks/server.md,
   "Accounts and the portal" in guides/hosting.md, and the invitation lines in
   guides/blocks/keys.md ("Inviting", "Contacts"); add the decision records
   below.

Registration admission (operator invite codes) stays a separate policy from the
credential kind and from member invitations. It is temporary, for while the
server is not safe enough to let anyone make an account, and for self-hosters
who don't want strangers signing up to their node, so it gets no UX work beyond
asking for the code when the server wants one.

## Other plans

- **Share links** (plans/share_links.md, in flight) let an account on the server
  open a block's link; the opener signs in or registers through this plan's
  portal like an invitee, and the host keeps the secret across it as for an
  invite link. Opening a link with no account at all is deferred
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

- Passwords are removed. Rejected: keeping them as a fallback, because it keeps
  the whole hashing and lockout machinery and weakens the passkey guarantee. A
  device with no browser signs in by opening the URL on another device, so none
  needs one.
- Login through a central provider only (rejected): breaks self-hosting, leaks
  metadata and makes us depend on someone else's recovery.
- An account that is only a bare keypair (rejected): fits end-to-end encryption
  but key loss has no safety net beyond the recovery phrase and it is hard to
  explain; passkeys give most of the benefit with platform sync.
- Invitations are links the inviter sends, never mail the server sends or
  anything bound to an email.
- Providers are configured by the operator, per server; accounts do not federate
  across servers.

## Open questions

- Emailed code or link? A link is easier on one device; a code works when the
  mail arrives on another. Probably offer a code that the link also fills in.
- Does the hosted server want Apple at launch, given the developer-account cost?
- Must every account keep a verified email? An account made through a provider
  has the provider's email, which may be a relay address. The contact card and
  "Connect with NAME (EMAIL)?" show an email, and the email is the recovery
  path, so an account whose only identity is a provider would have no recovery
  but the provider.
- With the email gone, anyone holding a leaked invite link joins as its role.
  Should accepting wait for the inviter to confirm the name on the accepter's
  card before the account becomes a member, for every invitation or only for
  administrators (who are sealed the escrow key)?
- How long does an invite link last before it expires?
