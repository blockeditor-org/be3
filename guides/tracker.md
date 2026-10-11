# The tracker

The project's issues are meant to live in block-app, as databases: an Issues
database whose rows are issues, with Relation columns to a Projects database and
a Milestones database, kept in a workspace of their own. Agents read and change them with `be`, the command line tool in
`crates/be-cli`, and GitHub moves them as pull requests open and merge.

## Using it from an agent

`be` needs the agent's credentials in the environment. In a Claude cloud
environment they are its secrets:

- `BE3_TOKEN`: the agent's token. Without it the connection must be signed in
  by a proxy that adds `Authorization: Bearer TOKEN` to the websocket upgrade.
- `BE3_RECOVERY_PHRASE`: opens the workspace key the server keeps sealed for the
  agent. Content never reaches the server unencrypted, so without it nothing can
  be read.
- `BE3_WORKSPACE`: the workspace, when the agent is in more than one.
- `BE3_SERVER`: the server, `https://blocks.pfg.pw` unless set.

Run it with `./scripts/buck run //crates/be-cli:be -- ARGS`; `be help` lists
everything. A command names a target and then a verb, and the verbs depend on
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
be 3fa9 cat                             a text block, reached directly
be Issues schema
be Issues add-column "Blocked by" relation --links Issues --multiple --inverse Blocking
be new-database Milestones --prefix MS --in a1f3
be Issues import issues.csv             Linear's export, or any CSV
```

A target is a block, by the start of its id as `be ls` prints it, its full id
or its name (a database view and the database inside it share a name, and the
name means the database), or a row by its id. A row's id is its database's
prefix and six characters made from the row's own id, so it never changes and
needs no coordination; the prefix is kept in the database's sealed metadata,
which is how `be` finds the database without opening the others, and two
databases with one prefix are refused. Field values are matched by name, case
aside: an option's name for an Enum column, a row's title or id for a Relation.
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
- **Views.** `DatabaseView::filters` hide the rows any filter rejects, in every
  view kind. The List kind is a line per row with a new-item box above, and a
  selected row opens beside it with its fields and its page, the page's own
  editor embedded live.
- **Agents.** An administrator adds one from the app menu (Agents). The app
  makes a recovery phrase for it and seals the workspace key to it, the server
  makes an editor account with no password and a token (`CreateAgent`), and
  the dialog shows the token and phrase once. An agent sees only what is shared
  with it, but the workspace key opens everything in the workspace, so keep the
  tracker in a workspace of its own. Removing the agent deletes its account and
  its tokens.
- **Writes.** `be` joins a block's live session when another client holds
  one, waits for the owner's snapshot, sends its edits as a follower and leaves
  once the owner has taken them, so they merge with unsaved typing the way a
  second window's would. With nobody in the session it opens the block, applies
  the same `Edit` the editor would and publishes against the head it read,
  retrying on the new head when the publish is rejected. A text write changes
  only the span that differs. Deleting a row also removes the links to it from
  the rows of its own database.
- **GitHub.** `.github/workflows/tracker.yml` runs `be pull-request` on the
  event: ids in the branch, title or body move to `BE3_REVIEW_STATUS` (In
  review) when the pull request opens, with a link on the page, and to
  `BE3_DONE_STATUS` (Done) when it merges. It reads the
  `BE3_TRACKER_TOKEN` and `BE3_TRACKER_RECOVERY_PHRASE` secrets and the
  `BE3_SERVER`, `BE3_WORKSPACE` and status variables, and builds `be` from the
  default branch.
- **Proxies.** `be-client` tunnels `wss` through `HTTPS_PROXY` and trusts the
  certificates in `SSL_CERT_FILE` as well as the usual roots, which is what a
  cloud environment's proxy needs.
