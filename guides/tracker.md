# The tracker

The project's issues are meant to live in block-app, as databases: an Issues
database whose rows are issues, with Relation columns to a Projects database and
a Milestones database, kept in a workspace of their own. Agents read and change them with `be`, the command line tool in
`crates/be-cli`, and GitHub moves them as pull requests open and merge.

## Using it from an agent

Run it as `./scripts/be ARGS`; `./scripts/be help` lists everything. That
script runs the `be` release pinned in `scripts/internal/common.sh`, with a
`build-id` file beside it naming the server's build, so the server refuses it
once it is deployed from another build and the pin has to move with it. It reads the agent's credentials from
`external/.tracker-credentials`, which git ignores and `./scripts/setup`
fetches from `https://blocks.pfg.pw/authorized-storage/tracker-credentials`
(behind the gateway, so only where the proxy adds its header), and
passes them to `be` as the `BE3_*` variables:

```text
server=https://blocks.pfg.pw
workspace=<the tracker workspace, when the agent is in more than one>
token=<the agent's token>
recovery_phrase=<opens the workspace key the server keeps sealed for the agent>
authorization=<an Authorization header a gateway in front of the server wants, if any>
```

`be` itself reads only the variables, never that file. A `be` built from the
checkout (`./scripts/buck run //crates/be-cli:be -- ARGS`) carries that
checkout's build id, so it talks to the dev app built from the same tree, and
the real server refuses it. `BE3_SERVER` is `https://blocks.pfg.pw` unless
set. `be` speaks to the server over HTTP posts (`/api/be/http`), since a cloud
environment's proxy refuses websockets; requests made together share one post.
It keeps the objects it reads in `~/.cache/be3/be/WORKSPACE` (`BE3_CACHE`),
still encrypted and named by their hash, so a later run reads only what
changed.

A command names a target and then a verb, and the verbs depend on
what the target is:

```text
be ls                                   the blocks at the root, each with a short id
be a1f3 ls                              a block's children
be Issues rows Status!=Done Project=Launch
be BE-7K3Q2M                            a row: its fields and its page
be Issues new "Fix the login" Status="To do" "Blocked by=BE-4HV8ND" --body "Steps..."
be BE-7K3Q2M set Status=Done "Blocked by-=BE-4HV8ND" --append "Fixed in #412."
be BE-7K3Q2M page edit                  its page in $EDITOR, merged with any change since
be BE-7K3Q2M delete
be 3fa9 delete                          a block and everything in it, to Recently Deleted
be 3fa9 cat                             a text block, reached directly
be Issues schema                        columns; each choice with its colour, icon, closed, default
be Issues option Status Done closed=yes icon=done color=blue
be Issues add-option Status Backlog default=yes first=yes
be Issues add-column "Blocked by" relation --links Issues --multiple --inverse Blocking
be new-database Milestones --prefix MS --in a1f3
be Issues import issues.csv             Linear's export, or any CSV
```

A target is a block, by the start of its id as `be ls` prints it, its full id
or (unless it is deleted) its name (a database view and the database inside it share a name, and the
name means the database), or a row by its id. A row's id is its database's
prefix and six characters made from the row's own id, so it never changes and
needs no coordination; the prefix is kept in the database's sealed metadata,
which is how `be` finds the database without opening the others, and two
databases with one prefix are refused. Field values are matched by name, case
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
JSON. An import's Title column is the title, Description the page, and every
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
  rows. The editor's header holds the views of the same database as tabs (the
  view blocks that reference it), search, New, filter chips and a Display menu
  for everything else; a selected row opens beside the list, board or scatter
  (over it on a phone) with its fields, its links and its page, the page's own
  editor embedded live.
- **Formats.** `Database`, `DatabaseSchema` and `DatabaseView` are frozen
  (`Root::FROZEN`): be-block's tests hold the format the first tracker build
  saved (`tests/frozen_database*.rs`) and check it still loads, so a change to
  these types only adds fields, variants and kinds (guides/the_new_block_stack.md,
  "The stored form").
- **Agents.** An administrator adds one from the app menu (Agents). The app
  makes a recovery phrase for it and seals the workspace key to it, the server
  makes an editor account with no password and a token (`CreateAgent`), and
  the dialog shows the token and phrase once. An agent sees only what is shared
  with it, but the workspace key opens everything in the workspace, so keep the
  tracker in a workspace of its own. Removing the agent deletes its account and
  its tokens.
- **Writes.** `be` opens the block, applies the same `Edit` the editor would
  and publishes against the head it read, retrying on the new head when the
  publish is rejected, so it never overwrites a save it did not see. An editor
  with the block open takes the new head the way it takes any outside publish,
  merging it with what it has not saved yet. A text write changes
  only the span that differs. Deleting a row also removes the links to it from
  the rows of its own database.
- **GitHub.** `.github/workflows/tracker.yml` runs `be pull-request` on the
  event: ids in the branch, title or body move to `BE3_REVIEW_STATUS` (In
  review) when the pull request opens, with a link on the page, and to
  `BE3_DONE_STATUS` when it merges, or, when that is unset, to the status
  column's first closed option (Done if it has none). It reads the
  `BE3_TRACKER_TOKEN`, `BE3_TRACKER_RECOVERY_PHRASE` and (for a gateway)
  `BE3_TRACKER_AUTHORIZATION` secrets and the
  `BE3_SERVER`, `BE3_WORKSPACE` and status variables, writes them to
  `external/.tracker-credentials` and runs `./scripts/be` from the default branch.
- **Releases.** `./scripts/deploy` (guides/hosting.md) uploads `be` with the
  server it deploys and pins it in `scripts/internal/common.sh`.
- **Proxies.** `be-client` tunnels `https` and `wss` through `HTTPS_PROXY` and
  trusts the certificates in `SSL_CERT_FILE` as well as the usual roots, which
  is what a cloud environment's proxy needs. That proxy may add the gateway's
  `Authorization` header itself, so `authorization=` can stay out of the file
  there.
