# Sign in with Apple

Depends on: none.

Set aside until the user picks it up.

Apple as a preset of the generic OIDC provider (guides/blocks/server.md, "Accounts").

- **Configuration**: the `.p8` key, team id and key id. The client secret is a
  JWT the server signs with the key, valid at most six months, so the server
  makes a fresh one before the old expires. Apple needs a paid developer
  account, the operator's cost, which is why the preset is optional.
- **The callback** comes with `response_mode=form_post`, a cross-site POST: the
  portal's `SameSite=Lax` cookie is not sent and its Origin check refuses the
  request. The Apple callback route takes no cookie and no Origin check; it
  finds its attempt from `state` in a server-side table (or a short-lived
  `SameSite=None` cookie set at start), and every other check the
  generic provider makes (`oidc.rs`: PKCE, nonce, the ID token's signature, `aud`, expiry)
  still applies.
- **Email**: Apple may give a relay address; it is authoritative for email, as
  `authoritative_email` means it, for the address it gives.
- **Name**: Apple sends the name only on first sign-in, so store it then.

Done: a test against a fake Apple endpoint where the form POST callback
completes without the cookie, and a second sign-in keeps the stored name.
