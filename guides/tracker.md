# The tracker

The project's issues live in be-host as databases: an Issues database with Relation columns to
Projects and Milestones, in a workspace shared with the agents that work on them. Agents read
and change them with `be` (`crates/be-tool-cli`), and GitHub moves rows as pull requests open
and merge.

## Running it

`./scripts/be ARGS`; `./scripts/be help` lists everything. The script runs the `be` release
pinned in `scripts/internal/pins` with its `build-info.json`, which names the server's build:
the server refuses a `be` from another build, so the pin moves with every deploy
(guides/hosting.md). A `be` built from the checkout
(`./scripts/buck run //crates/be-tool-cli:native -- ARGS`) talks only to a dev server built from
the same tree.

`be` uses HTTP posts (`/api/be/http`), since a cloud environment's proxy refuses websockets;
requests made together share one post. It caches the encrypted objects it reads in
`~/.cache/be3/be/ORGANIZATION` (`BE3_CACHE`). be-block-client tunnels through `HTTPS_PROXY` and
trusts `SSL_CERT_FILE`, which is what a cloud environment's proxy needs.

### Credentials

`be` reads `BE3_AGENT_CREDENTIALS`, the JSON copied from the host's Agents dialog
(`server`, `organization`, `token`, `recovery_phrase`; all secret). `./scripts/be` fetches it
on first use into `external/.tracker-credentials` (git-ignored) and passes it on. The bundle
overrides the separate `BE3_SERVER`, `BE3_ORGANIZATION`, `BE3_TOKEN` and `BE3_RECOVERY_PHRASE`.
`BE3_AUTHORIZATION` sets the gateway header; without it `./scripts/be` sends the key
`./scripts/buck` saved (guides/build_server.md), and a cloud proxy may add it itself. A bundle
with `"authentication": "proxy"` has no token: the proxy injects it (guides/hosting.md, Proxy
account credentials).

## Commands

A command names a target, then a verb that depends on what the target is:

```text
be ls                                   workspaces and what is shared with this account, with short ids
be Issues rows Status!=Done Project=Launch
be BE-7K3Q2M                            a row by id: its fields and page
be Issues new "Fix the login" Status="To do" "Blocked by=BE-4HV8ND" --body "Steps..."
be Issues row BE-7K3Q2M set Status=Done "Blocked by-=BE-4HV8ND" --append "Fixed in #412."
be Issues row BE-7K3Q2M page edit       its page in $EDITOR, merged with changes since
be Issues schema                        columns and options
be Issues add-column "Blocked by" relation --links Issues --multiple --inverse Blocking
be Issues import issues.csv             Linear's export, or any CSV
be Issues download issues.block         a block and everything in it, as one file
be upload issues.block --in a1f3        a copy, here or in another organization
be Code sync ~/code [--watch]           guides/sync.md
be a1f3 terminal-host                   guides/terminal.md
```

`--json` prints JSON. A target is a full id, the last eight hex digits `be ls` prints, or a
name, resolved by `Tracker::find`. Field values match option names, row titles or row ids,
ignoring case. A relation of several rows takes `FIELD=A,B`, `FIELD+=A` and `FIELD-=A`, and so
does its inverse column. `new-database` and `upload` without `--in` go into the one workspace the
account may write in.

### Row ids and prefixes

A row id is its database's prefix plus six characters derived from the row's own id, so it is
stable and needs no coordination. A row id alone resolves only through the prefix registries
(`be_block::PrefixRegistry`, a workspace settings kind, guides/settings.md); a prefix no
registry lists fails, and one shared by two databases asks you to name the database.
`new-database`, `upload` and the database editor register databases; moves keep the registries
right.

### Block files

`download` and `upload` read and write the block file format
(`crates/be-block-client/src/transfer.rs`), a frozen format (guides/frozen_formats.md) that
be-host's Export as and Import also use. `upload` gives every block a new id and rewrites ids
inside the file to match.

## Working on a row

When a task names a row, read it with `be ID` first, and put its id in the pull request's title,
body or branch name so GitHub moves it. File small problems you notice but don't fix as new
rows.

## How it fits together

- **Rows** are `be-block-model` objects in `Database`, so cells merge. A row's page is a text
  block under the database, made on first use. Multi-row relations keep each link as its own
  key (`DatabaseRow::links`) so concurrent additions both stay; an inverse column is computed
  (`inverses`), not stored.
- **Order.** Rows keep a manual order, the order of `Database::rows`: dragging a card or a list
  row (or Alt with an arrow key) moves it there with `ViewData::move_row`, and into another
  group sets its group value, in one edit. A sort decides the order instead, so a sorted list
  does not drag and a sorted board only carries cards between columns, opening the gap where
  the sort will put the card (`ViewData::slot_in`). Schema fields reorder the same way.
- **Writes.** `be` links the database app's types crate (`database_types`) and applies the same
  `Edit`s the editor does, publishing through `Tracker::edit`. Every other block type goes
  through its type module as bytes (`ToFile`, `Apply` of `FromFile`, `Change::Child`).
- **Agents.** Any member adds one from the host's Agents menu; only its creator lists or removes
  it. The server makes a passwordless editor account and token (`CreateAgent`); the host makes
  its recovery phrase and writes its public key into the creator's contacts, so the creator's
  devices can share with it. An agent sees only what is shared with it: share a workspace with
  it for `be` to write in. Removing the agent deletes its account and what was sealed to it.
- **A block's agent.** For a program that hosts one block from a machine trusted less than the
  person's devices, such as a terminal host in a cloud VM, an app with `"block_agents": true`
  calls `create-block-agent` for its own block or one below it (guides/apps/host_services.md).
  The server (`create-agent` with `bound-to`, which needs Manage on the block) grants the agent
  Edit there, and it reaches that block and the blocks under it and nothing else:
  `blocks::Visibility` answers no access outside them, so every key, object, listing and session
  answer is limited to them, and `client::bound_may` refuses the calls that reach past one block
  (agents, contacts but listing them, organizations, members, pairing, access, trashing, making a
  block outside it, moving the block itself). It never administers, and its role never changes.
  A block summary still names its parent and references, so their ids reach it. It is listed in
  the block's access list, not the Agents menu; a block has at most `MAX_AGENTS_PER_BLOCK` and a
  creator `MAX_BLOCK_AGENTS_PER_CREATOR`, apart from the creator's own agents. It reaches no
  further than its creator: once the creator no longer manages the block, or the agent loses
  its grant, or the block is collected, it is deleted (`forget_orphaned_block_agents`, after
  every grant, move, role, member or contact change and collection), and only a creator who
  manages the block rotates or removes it. While the block is in the trash its connections
  close and its calls are refused, and it stays for Restore. A new connection replaces its open
  one, which the server closes and whose sessions it leaves at once, and `rotate-agent-token`
  revokes every older token.
- **GitHub.** `.github/workflows/tracker.yml` runs `be DATABASE pull-request` from the default
  branch (never PR-head code) with `BE3_TRACKER_DATABASE`, `BE3_TRACKER_CREDENTIALS` and
  `BE3_TRACKER_AUTHORIZATION`: rows named in the branch, title or body move to
  `BE3_REVIEW_STATUS` when the PR opens and to `BE3_DONE_STATUS` (or the status column's first
  closed option) when it merges.
