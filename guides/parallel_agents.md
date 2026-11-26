# Running parallel subagents

How a coordinating agent splits a large task across subagents. Run each subagent as its own
cloud session: each has its own VM, so they don't compete for memory, disk, the buck2 daemon or
the headless be-host.

## Starting sessions

- `create_session` with `source_url` (the repository) and `source_revision` (the integration
  branch) starts a session on that branch, in the same environment. Omit `permission_mode`: the child
  inherits the coordinator's, and a mode more permissive than the coordinator's is refused.
- Run at most 5 builders and 2 reviewers at a time; start the next only after one finishes.
  More rarely helps, since verify is the bottleneck, and each idle one still costs a turn on
  its whole context whenever it is woken.
- Start a fresh session for each new task rather than handing a finished one its next. A
  long session pays for its whole context on every turn, and summarizing it loses detail; a
  fresh one starts from its prompt alone. The cost is what the old session had learned about
  the code, so the prompt carries what the task needs: the files, what was found or ruled
  out, the decisions already made, and the branch to start from. Keep a session only for
  follow-ups on its own branch (review fixes, CI failures, conflicts), and send a delta
  review to the reviewer of the head before it.
- A child can't read the coordinator's scratchpad, so each prompt carries the whole protocol:
  its branch, what to check, how to finish and what to report. Keep the shared part word for
  word the same in every prompt and put the task after it.
- Name the coordinator's session id (`get_session` with no id) so the child reports with
  `send_message`; ask for the report in its final reply too.
- A read-only reviewer must be told that `send_message` is allowed: "do not edit, commit or push" alone
  made one skip it. Tell it to load the tool first (`ToolSearch` with `select:mcp__claude-code-remote__send_message`)
  and to send the whole verdict as its last step. If a report never arrives, read the child's final
  reply with `list_events` (`kinds: ["result"]`) instead of waiting.
- A child that starts a background `./scripts/verify` ends its turn to wait, so `list_sessions` shows it
  `IDLE` with a result like "Verify is running in the background". It is still working (its
  `status_bucket` stays `WORKING`) and wakes when the command exits. Tell if it is done by the bucket
  (`COMPLETED`) or its latest `result`, not by `IDLE`. Ask each child to `send_message` the coordinator
  its final report only when it is truly finished, and never a progress note.
- One session and one distinct branch (`<prefix>/<task>`) per task. A child pushes only its
  branch and opens no pull request.
- Tell running children with `send_message` when something lands that they build on or must
  rebase over. `list_events` with `kinds: ["assistant", "result"]` reads a child's transcript.

## Saving tokens

Every turn re-reads a session's whole context, and a session idle for over an hour has lost
its prompt cache, so waking it re-reads that context uncached. So:

- Archive a builder once its branch lands, and a reviewer once its verdict is in. A reviewer
  reviews one branch in a fresh session; only a delta review of the same branch goes back to it.
- Send a session nothing it doesn't need to act on: no thanks, no "stand by", no progress
  replies. Fold the go-ahead, review fixes and next task into one message. Don't wake a
  session idle for over an hour unless the work is its own branch; give new work to a fresh
  session instead.
- Use a cheaper model (`model: "sonnet"`) or lower `effort` on `create_session` for mechanical
  work: rebasing, rerunning verify, stress-running a flake, reviewing a docs-only change. Keep
  the default for design and real bug work.
- Review only code. Docs-only commits, and commits that only apply a reviewer's own
  suggestions or must-fixes, land without review. A small branch gets a self-review against
  the prompt's checklist in the builder's final report instead of a reviewer.
- The coordinator keeps its running state (open branches, queue, session ids) in a file, and
  reads that rather than old transcripts.


- Only the coordinator moves and pushes the integration branch.
- A child finishes by running the targeted tests for its change (`./scripts/test`), pushing and
  reporting; it doesn't run `./scripts/verify`. A child whose verdict is to leave its work out
  pushes it anyway, for review.
- Land in batches: the coordinator cherry-picks every reviewed branch onto the integration tip
  in its own worktree and runs one `./scripts/verify` in the background over them all. If it
  passes, commit verify's autofixes and push. If it fails, find which branches the failures
  belong to (their crates, or verify again without them), push the rest and send only those
  back. Children then don't rebase and re-verify after every landing.
- To add a branch to a batch, cherry-pick it onto the tip; if it conflicts, ask the child to
  rebase onto the tip and push again, then add it to the next batch.
- Check `git status` after a batch's verify: verify accepts paintings and regenerates files, so
  a pass that left changes isn't clean until they are committed.
- Move commits with rebase or cherry-pick. `git reset --soft` onto a moved integration branch
  commits the old tree over it and silently reverts others' work. Check that
  `git diff <tip before> HEAD` touches only the agent's files.
- The permission classifier may refuse a child's force push after a rebase; take its local
  commit from its report and cherry-pick it.
- When the user reverses a decision that already landed, `git revert` it separately, so the
  change and its revert can be dropped together when tidying history.

## Verify and the build server

- A cold verify can run over ten minutes with no output. Tell children to run it in the
  background with a 45-minute timeout; with less, they give up on an empty log.
- After a `Cargo.toml` dependency change, the first verify regenerates the crates' facts and
  `Cargo.lock` and fails; rerun it.
- `./scripts/test` caches results, so rerunning a test proves nothing about flakiness; loop it
  within one run, and say how a flake was counted.
- Name a known flake in every prompt with what to do instead (push and say so if it is the only
  failure outside the child's crates), or every child stalls on it. Give one agent the fix.
- Build server outages ("service is currently unavailable", resets, dropped bootstrap
  downloads): a child reruns once, then commits as `wip:`, pushes, reports and stops. The
  coordinator reruns verify once itself; if that fails too, it pauses everyone and asks the user
  how to continue.
- When the usage limit is reached every session stops; message each to resume after it resets.

## Subagents on one VM

Worktrees on one VM work too but share its limits. Each agent makes its own worktree from the
integration branch's tip (`git -C /home/user/be3 worktree add /home/user/wt/<task> -b
<prefix>/<task> <integration-branch>`); the Agent tool's `isolation: "worktree"` has produced
stale bases.

- Memory: the VM has 15G and each worktree's buck2 daemon about 1.5G; run at most five, and have
  each run `./scripts/buck kill` when done.
- Disk: a `buck-out` can reach 11G; delete it after `./scripts/buck kill` (the build server
  caches everything).
- `pkill -x buck2` or `pkill -x be-host` kills every agent's process, not just the caller's.
- Give each agent driving be-host its own `BE_DRIVE_DIR` (`/tmp/be3-<task>`) and its own log
  directory, or they overwrite each other's.
