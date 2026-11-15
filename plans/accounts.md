# Accounts: sign-in without passwords

Depends on: none.

Today a remote account signs in, registers and recovers by a code mailed to its
email, or signs in with an OpenID Connect provider the operator configured or a
passkey it added; an operator invite code gates registration
(guides/blocks/server.md, "Accounts"). Passwords are gone
(decisions/accounts_have_no_passwords.md). The portal also changes an email
(`email_change_link`) and lists browser sessions. Mail goes through
`mail::Delivery`, whose only implementation, `LogDelivery`, writes every link to
the server log, the hosted server's included (guides/hosting.md, "Accounts and
the portal"). Agents and local profiles already have no password: a token is
their only credential. This plan adds sign-in methods beside the emailed code,
without requiring any one of them on every device or server.

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
   present; never required. The settings page names one by when it was added;
   a name the person gives, or one from its authenticator's AAGUID, waits until
   asked.
3. **OIDC identity `(issuer, sub)`.** Sign in with Google, Microsoft (one
   tenant) or an organization's own provider. Generic OIDC is the one
   implementation, single-tenant: each provider is configured with its own
   issuer, and multi-tenant ones (Microsoft's `common` or `organizations`) wait
   until asked. Google is a preset of it, and Apple a later one
   (plans/deferred/apple_sign_in.md). A server with no providers configured
   shows no such buttons.

An account needs no email: one whose identities are only providers or passkeys
is allowed, and those are its recovery. An account always keeps at least one
identity; the last can't be removed, and an account with only one (a provider
or a passkey alone) is nudged to add a second. An email verified by a provider
the operator marks authoritative for email becomes the account's
verified email. Where an email is shown for an account today, the contact card
and "Connect with NAME (EMAIL)?" among them, an account with no verified email
shows only its name.

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
- **Mail limits strangers can't spend.** Caps per address (an IPv6 address counted by its /64, as now, and
  also by its /48, so one allocation can't spread across many /64s), and a large
  per-account budget that refills quickly, counting only requests from addresses
  that never completed a sign-in for that account (a small, bounded remembered
  addresses table). A request for an email with no
  account does the same work and sends nothing, so total mail is at most the
  number of accounts times the per-account rate, and no global cap is needed
  (one reachable by anonymous traffic would lock everyone out). Registration
  codes are capped per invite code (and overall), and email-change links, which
  need a signed-in account, have their own global cap. Someone crowded out by a
  stranger sharing their address signs in with a passkey or a provider, which
  send no mail.
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
  removed mails a notice to the account's email; an account with no email gets
  the notice in the app, on its other sessions.
- **Recovery** has its own entry in the portal, "I can't sign in", apart from
  an ordinary code sign-in. Signing in through it, or through an operator login
  link, ends every other session by default, and counts as fresh proof.
- **Mail goes to the log.** Real delivery is deferred
  (plans/deferred/mail_delivery.md); `LogDelivery` stays, so the server's
  journal holds every live sign-in code and link, and whoever reads it can sign
  in to any account with an email.
  On the hosted server the operator login link (`be-server login-link`) is
  the documented way in.
- **Operator links.** `be-server login-link` makes a single-use link
  valid fifteen minutes, keeps only its hash, and builds it from
  `--auth-origin`. It is also the rescue for an account whose only identity was
  a provider the operator removed.

## Steps

1. **What is left of identities and emailed codes.** Codes sign in, register,
   recover and prove, and providers sign in (guides/blocks/server.md,
   Accounts). An account with no email has the empty string as its `email`
   everywhere, as a local profile does, and every client already shows only
   the name for it (decisions/an_account_with_no_email_has_an_empty_one.md).
   The portal's two cookies (`be3-account`, `be3-attempt`) are scoped to
   `/auth` without a prefix, so a sibling subdomain could set or shadow them;
   move both to `__Host-` names with `Path=/` once development serves HTTPS or
   the plain-HTTP setup uses a cookie name of its own. An account with no
   email is told nothing when an identity is added or removed: give it the
   notice in the app, on its other sessions, as the rules above say.
2. **Invitations stop depending on email.** Today an invitation is already a
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
Registration admission (operator invite codes) stays a separate policy from the
credential kind and from member invitations. It is temporary, for while the
server is not safe enough to let anyone make an account, and for self-hosters
who don't want strangers signing up to their node, so it gets no UX work beyond
asking for the code when the server wants one.

## Other plans

- **Share links** (plans/share_links.md) let an account on the server
  open a block's link; the opener signs in or registers through this plan's
  portal like an invitee, keeping the secret across it the same way. Opening a
  link with no account at all is deferred
  (plans/deferred/anonymous_share_links.md); what a server asks of an account
  made there with no identity is that plan's to answer.
- **Signed access** (plans/signed_access.md) puts the organization
  root's hash in the invitation, sealed with the code. Nothing here changes
  that; it strengthens the argument above, since records are signed with the
  phrase's key and an account takeover cannot sign them.
- **Local accounts** (plans/local_accounts.md) later gain a server "as pushing
  to a new remote"; that registers an account here and is part of that plan.

## Decisions to record

- Invitations are links the inviter sends, never mail the server sends or
  anything bound to an email, and they join as editors only.
