# An app owns its credentials for outside services

An app that reaches an outside service on the person's behalf (the boat app's boat.dev key)
keeps the credential itself, in a settings kind of its own type marked `KindVisibility::Owner`
(guides/settings.md, Owner only): readable by the person who pasted it alone, on any of their
devices, never by administrators or the server, and one per organization like every kind. The
app asks for the credential on a page of its own, checks it with the service before keeping it,
tracks its expiry and asks to reconnect when the service stops taking it. The host knows nothing
of the service: `fetch` adds no credentials (guides/apps/host_services.md).

## Why

- The host stays a general peer: a new service needs a new app, never a host change.
- Each service's keys, scopes, expiry and refusals differ; only the app that speaks the service
  knows what its errors mean and what a person must do about them.
- Owner-only settings already give the right reader (the person, never escrowed) and sync, so no
  second secret store is needed. Losing the recovery phrase loses the key, which costs only a
  reconnect.

## Rejected

- A host-wide credential store or OAuth broker: no outside service the apps use needs one yet,
  and boat.dev has no OAuth flow for third-party apps; it would be a second way to hold a secret.
- Keeping the key in the block's content: everyone the block is shared with would read it.
- Keeping it in the viewer's view state: a view block is an ordinary block, escrowed like any
  other, so the organization's administrators could recover it; an owner-only kind is not.
