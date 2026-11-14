# Settings

Depends on: none.

Settings have three tiers, all built: per user (a root `Settings` block),
per block with a workspace as the usual place (a block's `be_block_content::SETTINGS`
metadata points to its settings block, one block per kind in
`Settings::kinds`), and per organization (`be_block::organization_settings`,
whose `kinds` are defaults and `policies` policies). Kinds declare `FOLLOWS`,
`RULE` and `VISIBILITY` (the `Layered` trait in
`be-block-content/src/layered.rs`; the kinds in `be-block/src/display_settings.rs`,
`input_settings.rs`, `organization.rs` and `workspace_settings.rs`) and resolve through
`be_block::Resolution` (guides/settings.md). What is left is later kinds.

## Later

- **Apps.** Once apps can be installed: a workspace recommends apps
  without trusting them, so joining a workspace never runs a stranger's code.
  What runs is (apps the user trusts and apps the organization trusts)
  limited to the organization's allowlist.
- **Custom icons** as metadata pointing to an image block (a `BlockRef`
  value, like `TAGGED`). Today `be_block_content::metadata::ICON` holds only an icon
  name. An icon from a workspace's icon set is a reference, not a child, so it
  needs a kind open to anyone inside (`KindVisibility::Inside`).
- Database conventions shared across databases (status workflows, fields,
  saved views), instructions for agents working in a workspace, calendar and
  locale, default access for new blocks, a spell-check dictionary, and links to
  outside services.
