# Accounts sign in with identities the operator configures

An account on a server has an opaque id and a set of linked identities: a
verified email (a code mailed to it), passkeys, and OpenID Connect identities
`(issuer, sub)` from providers the server's operator lists
(guides/blocks/server.md, "Accounts"). Proving any one signs in. Each server is
its own account domain: accounts do not federate across servers, and a provider
is configured per server, one tenant's issuer each.

## Why

- Self-hosting rules out one central login provider: every server picks its
  own, and a server with none still works by emailed codes and passkeys.
- An account holds no content keys (workspace keys are sealed to the recovery
  phrase), so taking one over gets metadata and memberships, never plaintext
  nor a signature, and an email-based factor is strong enough as the baseline.
- Single-tenant providers keep the issuer the server checks one the operator
  chose; Microsoft's `common` or `organizations` would accept any tenant's
  tokens and need a tenant allowlist of their own.

## Rejected

- Login through a central provider only: it breaks self-hosting, leaks metadata
  to that provider and makes recovery depend on someone else.
- An account that is only a bare keypair: it fits end-to-end encryption, but
  losing the key has no safety net beyond the recovery phrase and it is hard to
  explain; passkeys give most of the benefit with platform sync.
- Accounts federated across servers: a server would trust another operator's
  sign-ins.
- Multi-tenant providers now: they wait until one is asked for.
