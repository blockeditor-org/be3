# The tracker

The project's issues are meant to live in block-app, as databases: an Issues
database whose rows are issues, with Relation columns to a Projects database and
a Milestones database, kept in a workspace and
shared with the agents that work on them. Agents read and change them with `be`, the command line tool in
`crates/be-cli`, and GitHub moves them as pull requests open and merge.

## Using it from an agent

Run it as `./scripts/be ARGS`; `./scripts/be help` lists everything. That
script runs the `be` release pinned in `scripts/internal/pins`, with a
`build-info.json` file beside it naming the server's build, so the server refuses it
once it is deployed from another build and the pin has to move with it. Set
`BE3_AGENT_CREDENTIALS` to the JSON copied from the app's Agents dialog, or save
that JSON in `external/.tracker-credentials`. The file is ignored by git and
`./scripts/be` fetches it on first use from
`https://api.be3.pfg.pw/authorized-storage/tracker-credentials` behind the gateway.
The wrapper passes it to `be` as `BE3_AGENT_CREDENTIALS`; `be` reads environment
variables, never that file.

```json
{"server":"https://api.be3.pfg.pw","organization":"<organization UUID>","token":"<agent token>","recovery_phrase":"<agent recovery phrase>"}
```

The bundle supplies all four connection fields together and takes precedence over
the individual `BE3_SERVER`, `BE3_ORGANIZATION`, `BE3_TOKEN` and
`BE3_RECOVERY_PHRASE` variables, which a directly invoked CLI also accepts.
`BE3_AUTHORIZATION` supplies a separate `Be3-Gateway-Authorization` header if needed; without it, `./scripts/be` sends the gateway key `./scripts/buck` saved (guides/build_server.md).
Select “Account token supplied by proxy” in the agent dialog to copy a bundle
without the account token. Configure the proxy to inject the shown
`Authorization: Bearer ...` value on every request to the server; the bundle’s
`authentication: "proxy"` selects header authentication in `be`. Treat the whole
bundle as secret. A `be` built from the
checkout (`./scripts/buck run //crates/be-cli:native -- ARGS`) carries that
checkout's build id, so it talks to the dev app built from the same tree, and
the real server refuses it. `BE3_SERVER` is `https://api.be3.pfg.pw` unless
set. `be` speaks to the server over HTTP posts (`/api/be/http`), since a cloud
environment's proxy refuses websockets; requests made together share one post.
It keeps the objects it reads in `~/.cache/be3/be/ORGANIZATION` (`BE3_CACHE`),
still encrypted and named by their hash, so a later run reads only what
changed.

A command names a target and then a verb, and the verbs depend on
what the target is:

```text
be ls                                   the workspaces and what is shared with this account, each with a short id
be workspaces                           the workspaces, and which it may write in
be a1f3 ls                              a block's children
be Issues rows Status!=Done Project=Launch
be BE-7K3Q2M                            a row, found through its workspace's prefixes: its fields and its page
be Issues row BE-7K3Q2M                 the same row, named within its database
be Issues new "Fix the login" Status="To do" "Blocked by=BE-4HV8ND" --body "Steps..."
be Issues row BE-7K3Q2M set Status=Done "Blocked by-=BE-4HV8ND" --append "Fixed in #412."
be Issues row BE-7K3Q2M page edit       its page in $EDITOR, merged with any change since
be Issues row BE-7K3Q2M delete
be 3fa9 move a1f3                       a block and everything in it, into another block
be 3fa9 delete                          a block and everything in it, to Recently Deleted
be 3fa9 cat                             a text block, reached directly
be Issues schema                        columns; each choice with its colour, icon, closed, default
be Issues option Status Done closed=yes icon=done color=blue
be Issues add-option Status Backlog default=yes first=yes
be Issues add-column "Blocked by" relation --links Issues --multiple --inverse Blocking
be new-database Milestones --prefix MS  in the one workspace it may write in (or --in a1f3)
be Issues import issues.csv             Linear's export, or any CSV
be Issues download issues.block         a block and everything inside it, as one file
be upload issues.block --in a1f3
                                        a copy of it, here or in another organization
                                        (without --in, where new-database puts one)
be Code sync ~/code [--watch]           a folder or workspace and a directory, kept in
                                        step both ways (guides/sync.md)
```

`be ls` lists the top: the organization's roots the account can see
(`ListChildren` of the root) and each block shared with it whose parent it
cannot see (`ListShared`, the app's Shared with me), never a settings block.
`be` never lists the whole organization: it lists the top, then a block's
children when it needs them, one request per block, and keeps what it has
listed for the rest of the command. `new-database` and `upload` without `--in`
go into the one workspace the account may write in, or, with none, the one
shared folder it may; with several they ask for `--in`. A target is a block, by
its full id (read directly), or by the end of its id as `be ls` prints it (the
last eight hex digits, which UUIDv7 makes random where its start is a
timestamp) or its name, found a level at a time from the top, all of a level's
listings sent together; the shallowest level with a match decides. A database
view and the database inside it share a name, and the name means the database. A row is
its id alone, `be ROW-ID [VERB]`, or named within its database, `be DATABASE row
ROW-ID [VERB]`. A row's id is its database's prefix and six characters made from the
row's own id, so it never changes and needs no coordination. The prefix is only a
label: two databases may share one, and a copy keeps it. A row id alone resolves only
through the prefix registries (`be_block::PrefixRegistry`, a workspace settings kind
any editor of the workspace changes): `be` reads the registry of each workspace at the
top and the open registry above each block shared with it, and takes the databases
registered with that prefix that it can view and that still lie inside the registry's
workspace. There is no search for a prefix beyond them; a prefix no registry lists
fails, and one that names more than one database says which, so name the database
instead. `new-database`, `upload`, and the database editor (when its prefix changes or
it is opened somewhere it is not yet listed) write the database into the nearest
registry above it, making the workspace's if there is none. Moving a block
(`Peer::set_parent`, which `be TARGET move`, `be sync` and the app's moves all go
through) moves the databases inside it from the registry above its old place to
the one above its new place. The editor's Display
menu says when another database there shares the prefix. Field values are matched by name, case
aside: an option's name for an Enum column, a row's title or id for a Relation.
`option FIELD OPTION` changes an option the column has, refusing any other
name, and `add-option` adds one; both take
`color=`, `icon=`, `closed=yes|no` (a closed option marks a row finished),
`default=yes|no`, `name=`, `after=OPTION` and `first=yes`. A new row, by `new`
or `import`, takes each Enum column's default option unless it sets that column.
A relation of several rows takes `FIELD=A,B`, `FIELD+=A` and `FIELD-=A`, and so
does its inverse column (`Blocking+=BE-…` writes `Blocked by` on that row). The
title is the first text column. `--body` replaces the row's page and `--append`
adds a paragraph to it; `-` reads either from standard input. `--json` prints
JSON. `download` writes a block file (`crates/be-client/src/transfer.rs`): each
block's type, metadata, references and decrypted content bytes, the outermost
block first and every block after its parent, into a new file only its owner
may read; a database inside a view is downloaded from the view. The app writes and reads the same file: the files
pane's Export as, Block file, and the block picker's Import tab. `upload` gives every block a new id and swaps the old
ids for the new ones in the content bytes (as 16 raw bytes, hyphenated or
simple hex) and references, so the copy points at its own children; ids of
blocks outside the file are left as they were. The file format is frozen (guides/frozen_formats.md). An import's Title column is the title, Description the page, and every
column named like a field sets it, adding the options an Enum column lacks and
the rows a Relation column names that its database lacks.

When a task names a row, read it with `be ID` first, and put its id in the
pull request's title or body (or the branch name) so GitHub moves it. File the
small problems you notice but do not fix as new rows rather than only listing
them in the handoff.

## How it fits together

- **Rows.** `Database` rows are `be-model` objects, so a row's id is stable and
  edits to different cells merge. `DatabaseRow::page` links the row's page, a
  text block created under the database the first time someone asks for one.
  `DatabaseValue::Relation(RowRef)` links a row of another database; the
  schema's `DatabaseRelationOptions` names which. A relation of several rows
  keeps each link as its own key of `DatabaseRow::links`, so links added at once
  both stay, and `DatabaseRow::get` reads them as `DatabaseValue::Links`. A
  relation can name an inverse column: `inverses` works out, for the database a
  relation points at, which rows link to each of its rows, and both the editor
  and `be` show and edit it as a column of its own. `short_id` and
  `Database::find_short_id` make and resolve ids.
- **Choices.** An Enum column's options (`EnumOption`) carry a `ChoiceColor`, a
  `ChoiceIcon` (a fixed set the editor maps to icon-library glyphs) and
  `closed`, which marks a row finished; `SchemaField::default_option` is what
  new rows start with. Filters match `IsOpen`/`IsClosed` against `closed`, and
  `AnyOf`/`NoneOf` against several values.
- **Views.** `DatabaseView::filters` hide the rows any filter rejects, in every
  view kind. A view groups its rows by `group_by` (an Enum or single-relation
  column, stored under the old `kanban_field` name; the board's columns), folds
  closed groups until opened, and hides the columns in `hidden_fields` from its
  rows. Rows keep a manual order, the order of `Database::rows`: dragging a
  card or a list row (or Alt with an arrow key) moves it there with
  `ViewData::move_row`, and into another group sets its group value, in one
  edit. A sort decides the order instead, so a sorted list does not drag and a
  sorted board only carries cards between columns. Schema fields reorder the
  same way. The editor's header holds the views of the same database as tabs (the
  view blocks that reference it), search, New, filter chips and a Display menu
  for everything else; a selected row opens beside the list, board or scatter
  (over it on a phone) with its fields, its links and its page, the page's own
  editor embedded live.
- **Formats.** `Database`, `DatabaseSchema` and `DatabaseView` are frozen
  (`Root::FROZEN`): be-block's tests hold the format the first tracker build
  saved (`tests/frozen_database*.rs`) and check it still loads, so a change to
  these types only adds fields, variants and kinds (guides/frozen_formats.md).
- **Agents.** Any member adds one from the app menu (Agents, in either a
  personal or company organization). Listing and removal are scoped to the
  creator; other members cannot list or remove the agent. The app makes a recovery phrase for it, the server
  makes an editor account with no password and a token (`CreateAgent`) and
  records the agent and its creator as contacts, and the app writes the agent's
  public key into its contact row, so every device of the creator can share
  with it; the dialog shows the JSON credentials once in a selectable text box, with a
  Copy credentials button. Share a workspace with
  it to write in (the files pane's New workspace, then Share), which is where
  `be` puts what it makes. An agent sees and opens
  only what is shared with it: sharing a block with it (`Peer::share`) makes the block a scope of its own and seals that scope's key
  to the agent (the stack guide's Keys), and `be` opens that key with the
  phrase, refusing a phrase that is not the account's. Removing the agent
  deletes its account, its tokens and what was sealed to it.
- **Writes.** `be` reads the block's head bytes, makes the new content and
  publishes it against the head it read (`Tracker::edit`), retrying on the new
  head when the publish is rejected, so it never overwrites a save it did not
  see. Every publish takes its references and derived metadata from the type
  module's `Describe`. An editor with the block open takes the new head the way
  it takes any outside publish, merging it with what it has not saved yet.
  Only databases, their schemas and views are edited as types: `be` links the
  database plugin's types crate (`database_types`) the way the database editor
  does, decodes rows, applies the same `Edit` the editor would and encodes
  them, since a tracker is an editor of rows with a command line rather than a
  host. Everything else goes through the type module as bytes: a page is read
  with `ToFile` and written with `Apply` of `Change::FromFile` (a text write
  changes only the span that differs), and a folder or workspace lists a child
  with `Change::Child`. The modules start on first use
  (`Tracker::types`), so `be ls` never compiles one. Deleting a row also
  removes the links to it from the rows of its own database.
- **GitHub.** `.github/workflows/tracker.yml` runs `be DATABASE pull-request`
  on the event, with the database's full id from the `BE3_TRACKER_DATABASE`
  variable: ids with that database's prefix in the branch, title or body move to `BE3_REVIEW_STATUS` (In
  review) when the pull request opens, with a link on the page, and to
  `BE3_DONE_STATUS` when it merges, or, when that is unset, to the status
  column's first closed option (Done if it has none). It reads the
  `BE3_TRACKER_CREDENTIALS` JSON secret and (for a gateway)
  `BE3_TRACKER_AUTHORIZATION`. It passes them as environment variables to
  `./scripts/be` from the default branch, alongside the status variables. The
  pinned CLI must support the bundle format; deploy the CLI and server together
  before configuring the new secret. The workflow never executes PR-head code.
- **Releases.** `./scripts/deploy` (guides/hosting.md) uploads `be` with the
  server it deploys and pins it in `scripts/internal/pins`.
- **Proxies.** `be-client` tunnels `https` and `wss` through `HTTPS_PROXY` and
  trusts the certificates in `SSL_CERT_FILE` as well as the usual roots, which
  is what a cloud environment's proxy needs. That proxy may add the gateway's
  `Be3-Gateway-Authorization` header itself, so `BE3_AUTHORIZATION` can stay unset there.
