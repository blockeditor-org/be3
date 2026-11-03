# Settings

Settings have three tiers, all built: per user (a root `Settings` block),
per block with a workspace as the usual place (a block's `be_block::SETTINGS`
metadata points to its settings block, one block per kind in
`Settings::kinds`), and per organization (`be_block::organization_settings`,
whose `kinds` are defaults and `policies` policies). Kinds declare `FOLLOWS`,
`RULE` and `VISIBILITY` (`be-block/src/settings.rs`,
`be-block/src/workspace_settings.rs`) and resolve through
`be_block::Resolution`. What is left is the server wipe that closes the
breaking changes, and later kinds.

## 1. Breaking changes, landed together before one server wipe

Saved data may change incompatibly here, as a one-time exception to
`guides/frozen_formats.md`: there is one server with data, and its content is
kept as a `.block` file. Block files don't store permissions, so a re-import
takes the default access. Every change of this section has landed (metadata
that points to a block, the access model with `Manage`, settings blocks and
the `ANYONE_INSIDE` grant on a kind). Remaining, unless it has already
happened outside the repository: wipe the server, re-import once, and update
`guides/frozen_formats.md` to say the break is over.

## Later

- **Apps.** Once apps can be installed: a workspace recommends apps
  without trusting them, so joining a workspace never runs a stranger's code.
  What runs is (apps the user trusts and apps the organization trusts)
  limited to the organization's allowlist.
- **Custom icons** as metadata pointing to an image block (a `BlockRef`
  value, like `TAGGED`). Today `be_block::metadata::ICON` holds only an icon
  name. An icon from a workspace's icon set is a reference, not a child, so it
  needs a kind open to anyone inside (`KindVisibility::Inside`).
- Database conventions shared across databases (status workflows, fields,
  saved views), instructions for agents working in a workspace, calendar and
  locale, default access for new blocks, a spell-check dictionary, and links to
  outside services.
