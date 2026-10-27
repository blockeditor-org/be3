# Running parallel subagents

How a coordinating agent splits a large task across subagents on one Claude Code cloud VM, and what goes wrong there. Learned from the canvas PR (plans/canvas.md steps 1-4), which ran up to seven agents at once, and the HTML core PR (plans/html_core.md).

## Shape of the work

- Keep the PR's branch checked out in `/home/user/be3` and never edit or build there. It is the integration branch: subagents land on it, the coordinator pushes it.
- Each subagent makes its own worktree from the integration branch's current tip, `git -C /home/user/be3 worktree add /home/user/wt/<task> -b canvas/<task> <integration-branch>`, and checks its base before starting. Don't use the Agent tool's `isolation: "worktree"`: it has produced worktrees on stale bases.
- A subagent lands by rebasing onto the integration branch and running `git -C /home/user/be3 merge --ff-only canvas/<task>`, retrying the rebase if someone landed first. Then it removes its worktree and branch. The coordinator pushes after each landing.
- Put the rules every subagent follows in one protocol file in the scratchpad and point each prompt at it, so a rule changed mid-run reaches every later agent. Keep a second notes file of the APIs each landed agent reports, for the agents that build on them.
- Run `./scripts/verify` again after any rebase onto a branch that moved. A fast-forward after only `//:check` once landed a lint failure, and paintings conflict only once work is combined.
- Give each agent its own log directory in the scratchpad (`<scratchpad>/<task>/`) and forbid shared names like `check.log`: agents writing the same file read each other's output as their own.
- Ask for the final report in the reply text. The handback tool sometimes refuses it, and the agent then ends with no report.
- Tell agents working near each other who they are, and send the remaining ones a short message when a neighbour lands something they must build on or rebase over.

## Limits of the VM

- Memory is the tightest limit. The VM has 15G and each worktree runs its own buck2 daemon of about 1.5G, and verify runs got OOM-killed with six worktrees building at once. Five concurrent agents is about the most that fits, and each must run `./scripts/buck kill` in its worktree when it finishes. The coordinator should kill its own idle daemon too.
- Disk is a fixed allowance of about 39G. A worktree's `buck-out` can reach 11G after verify. Delete `buck-out` when space runs low: everything is cached on the build server. Run `./scripts/buck kill` first, or buck2 reports a stale working directory.
- Never run `pkill -x buck2` or any other command that kills processes by name across the machine: it kills every agent's build.
- When the account's usage limit is reached, or the container restarts, every agent stops at once. Resume each one by messaging it (after the limit resets); its worktree and transcript survive. A restart kills background builds and buck2 daemons, so tell resumed agents to rerun theirs, and check each worktree's state (`git status`, `git log`) before trusting what its agent last reported.

## The build server

When one test target fails for reasons of the build server alone (a known relay error on one test run, say), name it in the protocol with the narrow check that replaces it: if it is verify's only failure and that target passes when run on its own with `./scripts/test`, the agent lands and says so in its report. Otherwise every agent stalls on the same flake.

It has intermittent outages: "service is currently unavailable", connection resets, broken pipes, keep-alive timeouts, jobs queued for over ten minutes, and dropped bootstrap downloads. Each agent reruns a failing command once. If it fails the same way again, the agent commits its work as `wip:`, keeps its worktree and stops with a report. The coordinator then runs `//:check` itself: if that passes, it resumes the agent; only if it fails too does it pause everything and ask the user. Run that check in the background, since a cold check can take longer than ten minutes.

## Permissions

In auto mode the permission classifier has refused a subagent's `merge --ff-only` and the coordinator's force-push as unreviewed or destructive, inconsistently. If it blocks landing, ask the user how to proceed rather than working around it.

## History

- Move commits only with rebase. Agents have twice squashed with `git reset --soft` onto the integration branch after it moved, which committed their old tree over it and silently reverted other agents' work. Squash only after rebasing, onto a fresh branch from the current tip (`git merge --squash`), and check that `git diff <tip before> HEAD` touches only the agent's own files before landing.
- When the user reverses a decision that already landed, have the agent `git revert` the commits separately, so the coordinator can drop the change and its revert together when it tidies history before the PR.

## Separate cloud sessions

A coordinator can instead run each agent as its own cloud session, with its own VM (15G of memory, 4 cores, about 30G of disk), which avoids the memory and disk limits above. Tested on this repository:

- `create_session` with `source_url` (the repository) and `source_revision` (the integration branch) starts a session checked out on that branch, in the same environment, with the given prompt. It runs commands without anyone approving them.
- `send_message` delivers a further message to it. The child can answer with its own `send_message` to `@parent`, which arrives in the coordinator as a notification. `list_events` reads its transcript; filter with `kinds: ["assistant", "result"]`.
- `get_session` shows whether it is working or idle, and `archive_session` releases its VM when it is done.
- Work moves only through git: each child pushes its own branch, and the coordinator fetches it and fast-forwards the integration branch, or has the child rebase and push again. Give each child a distinct branch name, and tell it not to open a pull request.
