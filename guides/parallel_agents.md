# Running parallel subagents

How a coordinating agent splits a large task across subagents, and what goes wrong. Run each subagent as its own cloud session: each has its own VM, so they don't compete for memory, disk, the buck2 daemon or the headless app, and they cost no more than subagents on one VM. Learned from the canvas PR (plans/canvas.md steps 1-4) and the HTML core PR (plans/html_core.md), which ran agents in worktrees on one VM, and the code-removal PR, which ran over thirty tasks as cloud sessions, up to fifteen at once.

## Starting a session

- `create_session` with `source_url` (the repository) and `source_revision` (the integration branch) starts a session checked out on that branch, in the same environment, with the given prompt. It runs commands without anyone approving them.
- A child cannot read the coordinator's scratchpad, so put the whole protocol in each prompt: its branch, what to check, how to finish and what to report. Keep the shared part word for word the same in every prompt, and put the task after it.
- Name the coordinator's session id in the prompt (`get_session` with no id gives it) so the child can report with `send_message`. Reports arrive in the coordinator as notifications; ask for the report in the child's reply text too.
- Give each task one session and one distinct branch (`<prefix>/<task>`). Tell the child it may push only that branch, never the integration branch, and never open a pull request.
- Send a running child a short `send_message` when something lands that it builds on or must rebase over, naming the commit and the new API.
- `list_events` reads a child's transcript; filter with `kinds: ["assistant", "result"]`, since a transcript is long. `get_session` shows whether it is working or idle.

## Landing

- Keep the integration branch checked out in the coordinator's repository. Only the coordinator moves it and pushes it.
- A child finishes by rebasing onto the integration branch, running `./scripts/verify`, pushing its branch and reporting. A child whose verdict is to leave its work out pushes its branch too, so the user can review it remotely.
- The coordinator fetches the branch and fast-forwards the integration branch when it can. When the integration branch moved after the child's base, compare the files the child changed with the files landed since its base (`git merge-base`, then `git diff --name-only` both ways). If they share none, or only a guide or a generated file such as `buck/cargo/crates.bzl` and `Cargo.lock`, and the rebase is clean, the coordinator rebases the child's commits itself and lands them. Otherwise it asks the child to rebase, rerun verify and push again.
- Because of that, run `./scripts/verify` on the integration branch yourself after every few landings, in a worktree of its own, in the background. Check `git status` there afterwards: verify accepts changed paintings and regenerates files, so a passing run that left changes is not clean.
- The permission classifier has sometimes refused a child's force push to its own branch after a rebase. Have the child report its local commit instead; if the commit applies cleanly, cherry-pick it.
- Move commits only with rebase or cherry-pick. Agents have squashed with `git reset --soft` onto an integration branch that had moved, which committed their old tree over it and silently reverted other agents' work. Check that `git diff <tip before> HEAD` touches only the agent's own files before landing.
- When the user reverses a decision that already landed, `git revert` its commits separately, so the coordinator can drop the change and its revert together when it tidies history before the PR.

## Verify and the build server

- A full `./scripts/verify` can take longer than ten minutes on a cold session and prints nothing until it ends. Tell children to run it in the background with a long timeout (45 minutes); several gave up on an empty log at ten or fifteen.
- After a `Cargo.toml` dependency changes, the first verify regenerates `crates.bzl` and `Cargo.lock` and fails; rerun it.
- `./scripts/test` caches results: a test that ran on the same tree and inputs is not run again. Rerunning one to see whether it is flaky proves nothing; loop it inside one run instead (a loop in the test, or the binary run directly), and say in a report how a flake was counted.
- When one test is a known flake, name it in every prompt with the rule that replaces it: if it is verify's only failure and the change doesn't touch its crate, the child pushes and says so. Otherwise every child stalls on the same flake. Give the flake to one agent to fix at its source.
- The build server has intermittent outages: "service is currently unavailable", connection resets, broken pipes, jobs queued for a long time, dropped bootstrap downloads. A child reruns a failing command once; if it fails the same way again, it commits its work as `wip:`, pushes and reports. The coordinator then runs verify itself before deciding whether to pause everything and ask the user.
- When the account's usage limit is reached every session stops at once. Resume each by messaging it after the limit resets; its checkout and transcript survive.

## Subagents on one VM

Agents in worktrees on one VM work too, but share its limits. Each makes its own worktree from the integration branch's tip (`git -C /home/user/be3 worktree add /home/user/wt/<task> -b <prefix>/<task> <integration-branch>`); don't use the Agent tool's `isolation: "worktree"`, which has produced worktrees on stale bases.

- Memory: the VM has 15G and each worktree's buck2 daemon takes about 1.5G; verify runs got OOM-killed with six worktrees building at once, so five is about the most. Each agent runs `./scripts/buck kill` in its worktree when it finishes.
- Disk: a worktree's `buck-out` can reach 11G after verify. Delete it when space runs low, after `./scripts/buck kill`; everything is cached on the build server.
- Never run `pkill -x buck2`, `pkill -x block-app` or anything that kills processes by name: it kills every agent's build or app.
- Every worktree's `:dev` uses the same `~/.cache/be3/dev` unless `BE_DRIVE_DIR` says otherwise. Give each agent that drives the app its own short `BE_DRIVE_DIR` (`/tmp/be3-<task>`).
- Give each agent its own log directory in the scratchpad and forbid shared names like `check.log`.
