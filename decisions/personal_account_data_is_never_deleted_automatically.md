# A personal account's data is never deleted automatically

Offline settings (`be_block::OfflineSettings`) let an organization's policy
say what a device does with what it kept after losing access (keep or delete)
and how long unsent edits wait first. That choice exists only for organization
accounts. For a personal account nothing is deleted automatically, unsent
edits included; a workspace not kept offline is forgotten from the refs
database but what is still waiting to be sent stays (`Peer::forget_offline`).

## Why

It is one of the settings plan's principles: what to delete on losing access
is an organization's data policy, and a personal account has no organization
to set one.
