// The merge queue's decisions, with nothing that talks to GitHub in them.
//
// Everything here is a pure function: current state goes in, one action comes
// out. tick.js is the half that reads the state and carries the action out.
// The split is what makes the queue testable without a repository to run
// against, and decide.test.js is the whole of the behaviour written down.

// Adding this is how something enters the queue, and it is also the whole of
// the queue's memory. Removing it is a cancellation, whoever removes it.
export const LABEL = 'merge-queue'

// Left behind when the queue gives up on a pull request, so that a list of
// open pull requests says which ones need someone to look at them. It is
// cleared the moment the author does something about it: a new push, or asking
// for the queue again.
export const FAILED_LABEL = 'merge-queue: failed'

// Every comment the queue writes carries one of these, invisible in the
// rendered body. It is how a stateless tick knows it has already said a thing:
// there is no database, so the comment history is the record.
const MARKER_PREFIX = 'merge-queue:'

export function marker(key) {
    return `<!-- ${MARKER_PREFIX}${key} -->`
}

export function markerKeyOf(body) {
    const match = /<!--\s*merge-queue:([^\s>]+)\s*-->/.exec(body ?? '')
    return match ? match[1] : null
}

// A comment is written once per state change. The caller passes the marker
// keys it found on the pull request; if ours is already among them, the state
// has been announced and this tick stays quiet.
function once(existingKeys, key, body) {
    if (existingKeys.includes(key)) return null
    return { key, body: `${body}\n\n${marker(key)}` }
}

// --- commands -------------------------------------------------------------

// Only a line that is nothing but the command counts. Someone quoting `/merge`
// while explaining the queue, or a review that mentions it in passing, is
// discussing it rather than asking for it, and a blockquote is someone else's
// words being repeated back.
export function parseCommand(body) {
    for (const rawLine of (body ?? '').split(/\r?\n/)) {
        const line = rawLine.trim()
        if (line.startsWith('>')) continue
        const match = /^\/merge(?:\s+(\S+))?$/.exec(line)
        if (!match) continue
        const argument = match[1]
        if (argument === undefined) return { kind: 'merge' }
        if (argument.toLowerCase() === 'cancel') return { kind: 'cancel' }
        return { kind: 'unknown', argument }
    }
    return null
}

// GitHub reports maintainers as `maintain` rather than as a kind of write, so
// asking for "write or higher" means naming the three that are.
export function hasWriteAccess(permission) {
    return ['admin', 'maintain', 'write'].includes(permission)
}

// Applying a label needs triage permission, which is one step below write, so
// the label on its own is not the authorisation. Anyone who can reach the
// label can be told no, and the label taken back off.
export function decideLabel(input) {
    const { label, sender, permission, isOwnAction } = input

    if (label !== LABEL) return { kind: 'ignore', reason: `${label} is not the queue's label` }
    if (isOwnAction) return { kind: 'ignore', reason: 'this queue applied the label itself' }

    if (!hasWriteAccess(permission)) {
        return {
            kind: 'reject',
            reason: `@${sender} has ${permission ?? 'no'} permission`,
            body: `@${sender}, the merge queue only takes pull requests from people with write access to this repository, so this label has been removed.`,
        }
    }

    return { kind: 'accept', reason: `queued by @${sender}` }
}

export function decideComment(input) {
    const { command, permission, isPullRequest, prState, hasLabel, commenter } = input

    if (!command) return { kind: 'ignore', reason: 'no command in the comment' }
    if (!isPullRequest) {
        return { kind: 'ignore', reason: 'the comment is on an issue, not a pull request' }
    }
    if (command.kind === 'unknown') {
        return {
            kind: 'reply',
            reason: `unknown subcommand \`${command.argument}\``,
            body: `\`/merge ${command.argument}\` is not a command. Use \`/merge\` to queue this pull request, or \`/merge cancel\` to take it out of the queue.`,
        }
    }
    if (!hasWriteAccess(permission)) {
        return {
            kind: 'reply',
            reason: `@${commenter} has ${permission ?? 'no'} permission`,
            body: `@${commenter}, the merge queue only takes commands from people with write access to this repository.`,
        }
    }
    if (prState !== 'open') {
        return {
            kind: 'reply',
            reason: `the pull request is ${prState}`,
            body: 'This pull request is already closed, so there is nothing to queue.',
        }
    }

    if (command.kind === 'merge') {
        if (hasLabel) return { kind: 'acknowledge', reason: 'already queued' }
        return { kind: 'enqueue', reason: `queued by @${commenter}` }
    }

    if (!hasLabel) return { kind: 'acknowledge', reason: 'not queued' }
    return {
        kind: 'cancel',
        reason: `cancelled by @${commenter}`,
        body: `Removed from the merge queue at @${commenter}'s request.`,
    }
}

// --- checks ---------------------------------------------------------------

// A check run that was skipped counts as a pass for a required status check as
// far as GitHub is concerned, which means a job that conditions itself out
// satisfies the rule without ever having run. That is the one failure mode
// this queue cannot afford, so a skip is a failure here and says so loudly
// rather than quietly waving the pull request through.
const CONCLUSION_PASSES = new Set(['success', 'neutral'])

function classifyRun(run) {
    if (run.status !== 'completed') return { state: 'pending' }
    if (CONCLUSION_PASSES.has(run.conclusion)) return { state: 'success' }
    if (run.conclusion === 'skipped') {
        return { state: 'failure', detail: 'was skipped, which this queue does not accept as a pass' }
    }
    return { state: 'failure', detail: run.conclusion ?? 'did not pass' }
}

function classifyStatus(status) {
    if (status.state === 'success') return { state: 'success' }
    if (status.state === 'pending') return { state: 'pending' }
    return { state: 'failure', detail: status.state }
}

// A check name can be reported more than once on one commit: a re-run adds a
// second run rather than replacing the first. The newest is the answer.
function newestByName(entries, nameOf, orderOf) {
    const newest = new Map()
    for (const entry of entries) {
        const name = nameOf(entry)
        const previous = newest.get(name)
        if (previous === undefined || orderOf(entry) >= orderOf(previous)) newest.set(name, entry)
    }
    return newest
}

export function summarizeChecks(required, checkRuns = [], commitStatuses = []) {
    const runs = newestByName(
        checkRuns,
        (run) => run.name,
        (run) => run.id ?? 0,
    )
    const statuses = newestByName(
        commitStatuses,
        (status) => status.context,
        (status) => Date.parse(status.updated_at ?? 0) || 0,
    )

    const failed = []
    const pending = []
    const missing = []

    for (const name of required) {
        const run = runs.get(name)
        const status = statuses.get(name)

        if (!run && !status) {
            missing.push(name)
            continue
        }

        const verdict = run ? classifyRun(run) : classifyStatus(status)
        if (verdict.state === 'failure') {
            failed.push({ name, detail: verdict.detail, url: run?.html_url ?? status?.target_url ?? null })
        } else if (verdict.state === 'pending') {
            pending.push(name)
        }
    }

    let state = 'success'
    if (failed.length > 0) state = 'failure'
    else if (pending.length > 0 || missing.length > 0) state = 'pending'

    return { state, failed, pending, missing }
}

// --- the queue ------------------------------------------------------------

// The queue is the label plus the order it was applied in, which the issue
// events API remembers for us. Two labels applied in the same second is a tie
// nothing in the data breaks, so the lower pull request number goes first and
// the order is at least stable across ticks.
export function orderQueue(entries) {
    return [...entries].sort((left, right) => {
        const byTime = (Date.parse(left.labeledAt) || 0) - (Date.parse(right.labeledAt) || 0)
        if (byTime !== 0) return byTime
        return left.number - right.number
    })
}

// --- position labels ------------------------------------------------------

// Purely something to look at. The queue's order is the order the merge-queue
// labels went on, read from the issue events every tick; these say what that
// order currently is, so that a list of open pull requests can be read without
// opening any of them. Nothing decides anything from them, and a tick that
// fails to write them still merges.
// Only the front of the queue is numbered exactly. Everything behind it shares
// one label, which is what stops the cost of a merge growing with the queue:
// when the head goes, the four numbered places all shift and the pull request
// promoted into the last of them changes, and that is four writes whether
// there are five pull requests waiting or fifty. The rest already say #5+ and
// go on saying it.
const NUMBERED_POSITIONS = 4

// Matches a trailing + as well, so that labels left by a different value of
// NUMBERED_POSITIONS are still recognised as this queue's and cleaned up.
const POSITION = /^#\d+\+?$/

export function positionLabel(index) {
    if (index < NUMBERED_POSITIONS) return `#${index + 1}`
    return `#${NUMBERED_POSITIONS + 1}+`
}

export function isPositionLabel(name) {
    return POSITION.test(name ?? '')
}

// The minimum set of label writes that makes the labels match the queue: what
// is already right is left alone, so a tick on a queue that has not moved
// writes nothing at all. A pull request carrying more than one of these - two
// ticks raced, or one died between the remove and the add - loses all but the
// right one.
export function decidePositions(queue) {
    const changes = []

    queue.forEach((entry, index) => {
        const wanted = positionLabel(index)
        const held = (entry.labels ?? []).filter(isPositionLabel)
        const stale = held.filter((name) => name !== wanted)
        const add = held.includes(wanted) ? null : wanted

        if (stale.length === 0 && add === null) return
        changes.push({ number: entry.number, add, remove: stale })
    })

    return changes
}

// --- the tick -------------------------------------------------------------

function checkList(names) {
    return names.map((name) => `\`${name}\``).join(', ')
}

export function decideTick(state) {
    const { queue, head } = state

    if (queue.length === 0) return { kind: 'idle', reason: 'the queue is empty' }
    if (!head) return { kind: 'idle', reason: 'the head of the queue could not be read' }

    const number = head.number
    const behind = queue.length - 1
    const context = behind > 0 ? ` (${behind} behind it)` : ''

    // Between listing the queue and reading the head, the head can be merged
    // or closed by someone else. The label outlives the pull request, so take
    // it off and let the next tick promote whatever is behind it.
    if (head.merged) {
        return { kind: 'unlabel', number, reason: 'already merged' }
    }
    if (head.state !== 'open') {
        return { kind: 'unlabel', number, reason: `the pull request is ${head.state}` }
    }

    // Checked here and not only when the label goes on, because the label is
    // what the queue reads and the label outlives the event that applied it.
    // A labelled pull request whose labelling event was never processed - the
    // run failed, the app token step failed, the webhook was never delivered -
    // would otherwise be merged by the next scheduled tick on nobody's
    // authority. This is the check that actually gates a merge.
    if (!head.queuedByAuthorized) {
        return {
            kind: 'dequeue',
            number,
            reason: `@${head.queuedBy ?? 'someone'} does not have write access`,
            comment: once(
                head.botComments,
                `dequeued-unauthorized-${head.queuedBy}`,
                `Removed from the merge queue: the \`${LABEL}\` label was applied by @${head.queuedBy ?? 'someone'}, who does not have write access to this repository.`,
            ),
        }
    }

    // The queue merges into one branch and reads one branch's rules. A pull
    // request aimed anywhere else would be measured against the wrong checks.
    if (head.baseRef !== head.defaultBranch) {
        return {
            kind: 'dequeue',
            number,
            failed: true,
            reason: `targets ${head.baseRef}, not ${head.defaultBranch}`,
            comment: once(
                head.botComments,
                `dequeued-base-${head.headSha}`,
                `Removed from the merge queue: it targets \`${head.baseRef}\`, and the queue only merges into \`${head.defaultBranch}\`.`,
            ),
        }
    }

    if (head.mergeable === false) {
        return {
            kind: 'dequeue',
            number,
            failed: true,
            reason: 'conflicts with the base branch',
            comment: once(
                head.botComments,
                `dequeued-conflict-${head.headSha}`,
                `Removed from the merge queue: this branch has conflicts with \`${head.defaultBranch}\`. Resolve them and comment \`/merge\` again.`,
            ),
        }
    }

    // GitHub computes mergeability lazily, and answers `null` until it has.
    // Nothing here is worth guessing at, and a later tick gets a real answer.
    if (head.mergeable === null || head.mergeable === undefined) {
        return { kind: 'wait', number, reason: `mergeability is not computed yet${context}` }
    }

    // A comparison that could not be read is not the same as being up to
    // date with the base branch, and merging is the one thing that cannot be
    // taken back.
    if (head.behindBy === null || head.behindBy === undefined) {
        return { kind: 'wait', number, reason: `could not compare #${number} with ${head.defaultBranch}${context}` }
    }

    // Behind the base branch is decided before the checks are looked at: the
    // checks on this head describe a merge base that has moved, so whatever
    // they say about it is already out of date.
    if (head.behindBy > 0) {
        return {
            kind: 'update-branch',
            number,
            expectedHeadSha: head.headSha,
            reason: `${head.behindBy} commit(s) behind ${head.defaultBranch}`,
            comment: once(
                head.botComments,
                `updating-${head.headSha}`,
                `Updating this branch with the latest \`${head.defaultBranch}\` (it was ${head.behindBy} commit(s) behind). It will merge once CI passes on the updated head.`,
            ),
        }
    }

    const checks = head.checks

    if (checks.state === 'failure') {
        const lines = checks.failed.map((failure) => {
            const what = failure.url ? `[${failure.name}](${failure.url})` : `\`${failure.name}\``
            return `- ${what} ${failure.detail}`
        })
        return {
            kind: 'dequeue',
            number,
            failed: true,
            reason: `required checks failed: ${checks.failed.map((failure) => failure.name).join(', ')}`,
            comment: once(
                head.botComments,
                `dequeued-failure-${head.headSha}`,
                ['Removed from the merge queue: a required check did not pass on `' + head.headSha.slice(0, 7) + '`.', '', ...lines, '', 'Push a fix and comment `/merge` again.'].join('\n'),
            ),
        }
    }

    if (checks.state === 'pending') {
        const waitingFor = [...checks.pending, ...checks.missing]
        return {
            kind: 'wait',
            number,
            reason: `waiting for ${checkList(waitingFor)}${context}`,
        }
    }

    return {
        kind: 'merge',
        number,
        sha: head.headSha,
        reason: `up to date with ${head.defaultBranch} and all required checks passed`,
    }
}

// --- merge outcomes -------------------------------------------------------

// The merge call is the queue's one atomic step, and the ruleset's "require
// branches to be up to date" is what makes it one: if main moved between the
// decision and the call, GitHub refuses rather than merging something stale.
// A refusal for that reason is the design working, so it is not an error to
// report, it is a tick that ends early and gets recomputed.
const TRANSIENT_MERGE_FAILURE =
    /was modified|out of date|not up to date|not mergeable|merge already in progress|try merging again/i

export function classifyMergeFailure(status, message = '') {
    if (status >= 500) return 'transient'
    if (status === 409) return 'transient'
    if (TRANSIENT_MERGE_FAILURE.test(message)) return 'transient'
    return 'fatal'
}

export function mergeFailureAction(number, headSha, status, message, botComments = []) {
    if (classifyMergeFailure(status, message) === 'transient') {
        return {
            kind: 'wait',
            number,
            reason: `the merge call was refused as a race (${status}: ${message}); the next tick will re-evaluate`,
        }
    }
    return {
        kind: 'dequeue',
        number,
        failed: true,
        reason: `the merge call was refused (${status}: ${message})`,
        comment: once(
            botComments,
            `dequeued-merge-${headSha}`,
            `Removed from the merge queue: GitHub refused the merge.\n\n> ${message}\n\nFix that and comment \`/merge\` again.`,
        ),
    }
}
