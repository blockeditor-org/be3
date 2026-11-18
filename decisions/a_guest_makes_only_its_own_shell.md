# A guest makes only its own shell

A guest of an organization (an account a share link admitted from outside it,
guides/blocks/keys.md, "Share links") runs be-host there as anyone does, and
be-host's shell keeps the account's state in blocks of that organization: a
`Settings` block at the top, and under it the device profiles and sessions. So a
guest may make exactly those: a `Settings` at the top that is `owner-only`
(readable by it alone, its key sealed to it and never escrowed, granted to no
one), and below it whatever its shell keeps, which that block's scope seals to
the guest alone. It makes nothing else: no block at the top of any other type,
none a member could read, none inside a block someone else made, even one its
link lets it edit. The server checks it by the block's type, place and
`owner-only` mark (`require_guest_shell` in `be-server/src/blocks.rs`), not by
anything the device says. When the guest leaves (its last grant falls), its
shell's blocks are deleted with it (`share_links::drop_idle_guest`).

be-host skips My files and Unfiled for a guest, and its files pane shows only
Shared with me, which holds everything the guest reaches there; the New button
is hidden. Apps learn the account's standing from the `role-watch`
(`account-role`: member, administrator, guest).

## Why

Before this, a guest's be-host made its settings, My files and Unfiled as in
any organization; the server refused each, and the shell ran on local copies the
server would never hold, so an editor could wait on one for good (the flaky
`an_outsider_opens_a_share_link_as_a_guest`). Letting a guest keep its own
shell's blocks, which no one else can read, is the smallest change that keeps one
way of running a shell in every organization: the shell, its sessions and
profiles, and the settings they resolve work unchanged.

## Rejected

- Never opening a guest's organization in be-host, and showing what is shared
  with a guest in its own organization instead: the mirror, the connection and
  the shell are per organization, so the shell would have to reach across
  organizations for every shared block, a much larger change for the same view.
- Keeping a guest's shell blocks on the device only, never sent: a second kind
  of block the stack would not publish, kept apart from every other block, and
  lost to the guest's other devices.
