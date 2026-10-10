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
the commands.

```text
be databases
be list Issues Status!=Done Project=Launch
be show BE-7K3Q2M
be new Issues "Fix the login" Status="To do" Project=Launch --body "Steps..."
be set BE-7K3Q2M Status=Done --append "Fixed in #412."
```

A row's id is its database's prefix and six characters made from the row's own
id, so it never changes and needs no coordination. Field values are matched by
name, case aside: an option's name for an Enum column, a row's title or id for
a Relation. The title is the first text column. `--body` replaces the row's page
and `--append` adds a paragraph to it; `-` reads either from standard input, and
`be edit` opens the page in `$EDITOR` and merges the edit with whatever changed
meanwhile. `--json` prints JSON. `be import Issues issues.csv` adds a row for
each line of a CSV file such as Linear's export: its Title column is the title,
Description the page, and every column named like a field sets it, adding the
options an Enum column lacks.

When a task names a row, read it with `be show` first, and put its id in the
pull request's title or body (or the branch name) so GitHub moves it. File the
small problems you notice but do not fix as new rows rather than only listing
them in the handoff.

## How it fits together

- **Rows.** `Database` rows are `be-model` objects, so a row's id is stable and
  edits to different cells merge. `DatabaseRow::page` links the row's page, a
  text block created under the database the first time someone asks for one.
  `DatabaseValue::Relation(RowRef)` links a row of another database; the
  schema's `DatabaseRelationOptions` names which. `short_id` and
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
- **Writes.** `be` is a peer without a session: it opens a block, applies the
  same `Edit` the editor would, and publishes against the head it read; a
  rejected publish is retried on the new head. An app editing the same block
  merges it in as it would any publish from outside its session.
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
