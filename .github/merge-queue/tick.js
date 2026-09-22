#!/usr/bin/env node
// The merge queue, as one idempotent pass.
//
// Every run reads the current state of the repository from scratch and takes
// at most one queue action. There is no stored state: the queue is the set of
// open pull requests carrying the merge-queue label, ordered by when the label
// was applied, and everything else is read back from GitHub each time. That is
// what makes it safe for Actions to drop superseded runs, and what makes a
// crashed run cost nothing but the time until the next trigger.
//
// The decisions live in decide.js and are pure. This file is the part that
// reads the world and carries out what was decided.

import { readFileSync } from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'

import { ApiError, GitHub } from './github.js'
import {
    FAILED_LABEL,
    LABEL,
    decideComment,
    decideLabel,
    decidePositions,
    hasWriteAccess,
    isPositionLabel,
    decideTick,
    markerKeyOf,
    mergeFailureAction,
    orderQueue,
    parseCommand,
    summarizeChecks,
} from './decide.js'

const environment = process.env
const [owner, repo] = (environment.GITHUB_REPOSITORY ?? '').split('/')
const eventName = environment.GITHUB_EVENT_NAME ?? 'workflow_dispatch'
const appSlug = environment.MERGE_QUEUE_APP_SLUG ?? ''
const botLogin = appSlug ? `${appSlug}[bot]` : null
const fallbackChecks = (environment.MERGE_QUEUE_REQUIRED_CHECKS ?? '')
    .split(',')
    .map((name) => name.trim())
    .filter(Boolean)

const github = new GitHub({ token: environment.MERGE_QUEUE_TOKEN, owner, repo })

const event = (() => {
    const path = environment.GITHUB_EVENT_PATH
    if (!path) return {}
    try {
        return JSON.parse(readFileSync(path, 'utf8'))
    } catch {
        return {}
    }
})()

function heading(text) {
    console.log(`\n=== ${text} ===`)
}

function log(...parts) {
    console.log(...parts)
}

// --- reading the world ----------------------------------------------------

async function getDefaultBranch() {
    const repository = await github.get(github.repoPath(''))
    return repository.default_branch
}

// Asking the branch what it requires, rather than repeating the answer here,
// is what keeps the queue honest when someone adds a job to the gate. The
// rules endpoint answers for anyone who can read the repository; the classic
// protection endpoint needs admin, so it is only a fallback and a 403 from it
// is not an error.
async function getRequiredChecks(branch) {
    const contexts = []
    let strict = false

    const rules = await github.getOrNull(github.repoPath(`/rules/branches/${encodeURIComponent(branch)}`))
    for (const rule of rules ?? []) {
        if (rule.type !== 'required_status_checks') continue
        if (rule.parameters?.strict_required_status_checks_policy === true) strict = true
        for (const check of rule.parameters?.required_status_checks ?? []) contexts.push(check.context)
    }

    if (contexts.length === 0) {
        const protection = await github.getOrNull(
            github.repoPath(`/branches/${encodeURIComponent(branch)}/protection/required_status_checks`),
            [403, 404],
        )
        if (protection) {
            contexts.push(...(protection.contexts ?? []))
            if (protection.strict === true) strict = true
        }
    }

    const unique = [...new Set(contexts)]
    if (unique.length > 0) {
        log(`required checks on ${branch}: ${unique.join(', ')}`)
    } else if (fallbackChecks.length > 0) {
        log(`WARNING: ${branch} declares no required status checks; falling back to ${fallbackChecks.join(', ')}`)
        unique.push(...fallbackChecks)
    }

    // The atomicity of this whole design is one property of the ruleset: with
    // "require branches to be up to date" on, a merge call loses the race
    // rather than winning it with a stale head. Without it the queue still
    // merges, but nothing stops it merging something whose checks described a
    // different main.
    if (!strict) {
        log(`WARNING: ${branch} does not require branches to be up to date before merging.`)
        log('         The queue relies on that to make each merge atomic. Turn it on in the ruleset.')
    }

    return unique
}

// The order the labels were applied in is the queue, and the issue events API
// is where that is written down.
// The same scan answers both questions the queue asks about a label: when it
// went on, which is the order, and who put it there, which is the authority.
// The second is free here, where asking the API who labelled something
// otherwise is not.
async function labelApplication(number) {
    const events = await github.paginate(github.repoPath(`/issues/${number}/events`))
    let application = null
    for (const entry of events) {
        if (entry.event === 'labeled' && entry.label?.name === LABEL) {
            application = { at: entry.created_at, by: entry.actor?.login ?? null }
        }
        if (entry.event === 'unlabeled' && entry.label?.name === LABEL) application = null
    }
    return application
}

// The bot labels a pull request only after decideComment has approved the
// person who asked, so its own labelling carries that approval forward rather
// than being an unattributable one.
async function authorizedToQueue(login) {
    if (!login) return false
    if (botLogin && login === botLogin) return true
    return hasWriteAccess(await permissionOf(login))
}

async function getQueue() {
    const issues = await github.paginate(github.repoPath(`/issues?labels=${encodeURIComponent(LABEL)}&state=open`))
    const entries = []
    for (const issue of issues) {
        if (!issue.pull_request) continue
        const application = await labelApplication(issue.number)
        entries.push({
            number: issue.number,
            title: issue.title,
            labeledAt: application?.at ?? issue.updated_at,
            labeledBy: application?.by ?? null,
            labels: (issue.labels ?? []).map((label) => label.name ?? label),
        })
    }
    return orderQueue(entries)
}

async function botCommentKeys(number) {
    const comments = await github.paginate(github.repoPath(`/issues/${number}/comments`))
    return comments.map((comment) => markerKeyOf(comment.body)).filter(Boolean)
}

async function hydrate(entry, defaultBranch, required) {
    let pullRequest = await github.get(github.repoPath(`/pulls/${entry.number}`))

    // mergeable is computed on demand and comes back null until GitHub has
    // done it. One nudge is worth it, because the alternative is a tick that
    // does nothing; past that, waiting for the next tick costs nothing.
    if (pullRequest.mergeable === null && pullRequest.state === 'open') {
        log(`mergeability of #${entry.number} is not computed yet; asking once more`)
        await sleep(4000)
        pullRequest = await github.get(github.repoPath(`/pulls/${entry.number}`))
    }

    const headSha = pullRequest.head.sha

    // Not getOrNull with a zero default: a comparison that could not be read
    // is not a comparison that came back "up to date". A head that was
    // force-pushed out from under this tick answers 404 here, and treating
    // that as zero commits behind is how a stale branch gets merged.
    const comparison = await github.getOrNull(
        github.repoPath(`/compare/${encodeURIComponent(defaultBranch)}...${headSha}`),
    )

    const checkRuns = await github.paginate(github.repoPath(`/commits/${headSha}/check-runs`))
    const combined = await github.getOrNull(github.repoPath(`/commits/${headSha}/status`))

    return {
        number: entry.number,
        title: pullRequest.title,
        state: pullRequest.state,
        merged: pullRequest.merged === true,
        headSha,
        baseRef: pullRequest.base.ref,
        defaultBranch,
        mergeable: pullRequest.mergeable,
        behindBy: comparison ? comparison.behind_by : null,
        queuedBy: entry.labeledBy,
        queuedByAuthorized: await authorizedToQueue(entry.labeledBy),
        checks: summarizeChecks(required, checkRuns, combined?.statuses ?? []),
        botComments: await botCommentKeys(entry.number),
    }
}

// --- writing to the world -------------------------------------------------

const LABELS = [
    { name: LABEL, color: '1f6feb', description: 'Queued to merge into the default branch' },
    { name: FAILED_LABEL, color: 'd73a4a', description: 'The merge queue gave up on this; see the latest comment' },
]

async function ensureLabels() {
    for (const label of LABELS) {
        const existing = await github.getOrNull(github.repoPath(`/labels/${encodeURIComponent(label.name)}`))
        if (existing) continue
        log(`creating the ${label.name} label`)
        await github.request('POST', github.repoPath('/labels'), { body: label })
    }
}

async function addLabel(number, name = LABEL) {
    await github.request('POST', github.repoPath(`/issues/${number}/labels`), { body: { labels: [name] } })
}

async function removeLabel(number, name = LABEL) {
    try {
        await github.request('DELETE', github.repoPath(`/issues/${number}/labels/${encodeURIComponent(name)}`))
    } catch (error) {
        if (error instanceof ApiError && error.status === 404) return
        throw error
    }
}

// The failure label answers "does this need me?" in a list of pull requests,
// so it stops being true the moment its author does something about it.
async function clearFailed(number) {
    await removeLabel(number, FAILED_LABEL)
}

async function clearPosition(number) {
    const issue = await github.getOrNull(github.repoPath(`/issues/${number}`))
    for (const label of issue?.labels ?? []) {
        const name = label.name ?? label
        if (isPositionLabel(name)) await removeLabel(number, name)
    }
}

// Every way out of the queue goes through here, so that a position label
// cannot outlive the queue entry it describes. The position is cosmetic and
// the label is not, so the label goes first: if this run dies in between, the
// pull request is out of the queue with a stale number on it, which the next
// tick corrects, rather than in the queue with no number, which it would not.
async function leaveQueue(number) {
    await removeLabel(number, LABEL)
    await clearPosition(number)
}

// Created on demand, because how many there are is however many are queued.
async function ensurePositionLabel(name) {
    const existing = await github.getOrNull(github.repoPath(`/labels/${encodeURIComponent(name)}`))
    if (existing) return
    await github.request('POST', github.repoPath('/labels'), {
        body: { name, color: 'ededed', description: 'Position in the merge queue' },
    })
}

// Cosmetic, and treated as such by its caller: this throwing must not stop a
// pull request merging.
async function syncPositions(queue) {
    const changes = decidePositions(queue)
    if (changes.length === 0) {
        log('queue positions are already correct')
        return
    }
    for (const change of changes) {
        for (const name of change.remove) await removeLabel(change.number, name)
        if (change.add) {
            await ensurePositionLabel(change.add)
            await addLabel(change.number, change.add)
        }
        log(`#${change.number} is now ${change.add ?? 'unnumbered'}`)
    }
}

async function comment(number, body) {
    await github.request('POST', github.repoPath(`/issues/${number}/comments`), { body: { body } })
}

async function react(commentId) {
    await github.request('POST', github.repoPath(`/issues/comments/${commentId}/reactions`), {
        body: { content: 'eyes' },
    })
}

// --- the comment commands -------------------------------------------------

// The queue's own update-branch call and its own labelling both arrive back
// as events, and acting on them would mean undoing what was just done: every
// branch update would dequeue what it updated, and every label the queue
// applied would be checked against a bot's permissions and taken off again.
// When the app's own login is unknown, any bot gets the benefit of the doubt
// rather than none.
function isOwnAction() {
    const sender = event.sender?.login ?? ''
    if (!botLogin) {
        log('WARNING: MERGE_QUEUE_APP_SLUG is not set, so this queue cannot tell its own actions from other bots.')
        return event.sender?.type === 'Bot'
    }
    return sender === botLogin
}

async function permissionOf(login) {
    const result = await github.getOrNull(github.repoPath(`/collaborators/${encodeURIComponent(login)}/permission`))
    return result?.permission ?? 'none'
}

async function handleComment() {
    heading('comment')

    const issue = event.issue ?? {}
    const body = event.comment?.body ?? ''
    const command = parseCommand(body)

    if (!command) {
        log('no /merge command in this comment')
        return
    }

    const commenter = event.comment?.user?.login ?? 'someone'
    const isPullRequest = Boolean(issue.pull_request)
    const permission = isPullRequest ? await permissionOf(commenter) : 'none'
    const hasLabel = (issue.labels ?? []).some((label) => label.name === LABEL)

    const action = decideComment({
        command,
        permission,
        isPullRequest,
        prState: issue.state,
        hasLabel,
        commenter,
    })

    log(`#${issue.number} @${commenter} (${permission}) said /merge${command.kind === 'cancel' ? ' cancel' : ''}`)
    log(`-> ${action.kind}: ${action.reason}`)

    if (action.kind === 'ignore') return

    if (action.kind === 'reply') {
        await comment(issue.number, action.body)
        return
    }

    await react(event.comment.id)

    if (action.kind === 'enqueue') {
        await clearFailed(issue.number)
        await addLabel(issue.number)
    }
    if (action.kind === 'cancel') {
        await leaveQueue(issue.number)
        await comment(issue.number, action.body)
    }
}

// Adding the label is what puts a pull request in the queue, so this is where
// the permission check for that lives. The label itself cannot be the
// authorisation: applying one needs triage permission, which is a step below
// the write access the queue asks for.
async function handleLabeled() {
    heading('label')

    const pullRequest = event.pull_request ?? {}
    const label = event.label?.name ?? ''
    const sender = event.sender?.login ?? 'someone'

    const action = decideLabel({
        label,
        sender,
        permission: isOwnAction() ? null : await permissionOf(sender),
        isOwnAction: isOwnAction(),
    })

    log(`#${pullRequest.number} ${label} added by @${sender}`)
    log(`-> ${action.kind}: ${action.reason}`)

    if (action.kind === 'ignore') return

    if (action.kind === 'reject') {
        await leaveQueue(pullRequest.number)
        await comment(pullRequest.number, action.body)
        return true
    }

    await clearFailed(pullRequest.number)
    return false
}

// A human pushing to a queued branch has changed what the queue was about to
// merge, so the queue lets go of it rather than racing them. The branch update
// the queue itself performs arrives here as the same event, which is why the
// sender is checked: without that, every update would dequeue what it updated.
async function handleSynchronize() {
    heading('push to a queued branch')

    const pullRequest = event.pull_request ?? {}
    const sender = event.sender?.login ?? ''
    const hasLabel = (pullRequest.labels ?? []).some((label) => label.name === LABEL)

    if (!hasLabel) {
        log(`#${pullRequest.number} is not queued; clearing any failure label`)
        await clearFailed(pullRequest.number)
        return
    }
    if (isOwnAction()) {
        log(`#${pullRequest.number} was updated by ${sender}, which is this queue; keeping it queued`)
        return
    }

    log(`#${pullRequest.number} got new commits from @${sender}; dequeueing`)
    await leaveQueue(pullRequest.number)
    await clearFailed(pullRequest.number)
    await comment(
        pullRequest.number,
        `Removed from the merge queue: @${sender} pushed new commits while it was queued. Comment \`/merge\` again when it is ready.`,
    )
}

// The queue only ever lists open pull requests, so a closed one leaves it by
// itself. Taking the label off is tidiness, so that the label means what it
// says wherever it is read from.
async function cleanupClosed() {
    const closed = await github.paginate(
        github.repoPath(`/issues?labels=${encodeURIComponent(LABEL)}&state=closed`),
    )
    for (const issue of closed) {
        if (!issue.pull_request) continue
        log(`#${issue.number} is closed; removing the ${LABEL} label`)
        await leaveQueue(issue.number)
    }
}

// --- carrying out the decision --------------------------------------------

async function updateBranch(action) {
    try {
        await github.request('PUT', github.repoPath(`/pulls/${action.number}/update-branch`), {
            body: { expected_head_sha: action.expectedHeadSha },
        })
    } catch (error) {
        if (error instanceof ApiError) {
            log(`the branch update was refused (${error.status}: ${error.apiMessage}); the next tick will re-evaluate`)
            return
        }
        throw error
    }
    log(`updated #${action.number} with the latest base branch`)
    if (action.comment) await comment(action.number, action.comment.body)
}

async function merge(action, head) {
    try {
        await github.request('PUT', github.repoPath(`/pulls/${action.number}/merge`), {
            body: { merge_method: 'squash', sha: action.sha },
        })
    } catch (error) {
        if (!(error instanceof ApiError)) throw error
        const outcome = mergeFailureAction(action.number, head.headSha, error.status, error.apiMessage, head.botComments)
        log(`-> ${outcome.kind}: ${outcome.reason}`)
        if (outcome.kind === 'dequeue') {
            await leaveQueue(outcome.number)
            if (outcome.failed) await addLabel(outcome.number, FAILED_LABEL)
            if (outcome.comment) await comment(outcome.number, outcome.comment.body)
        }
        return
    }

    log(`merged #${action.number}`)
    await leaveQueue(action.number)
    await comment(action.number, `Merged by the merge queue at \`${action.sha.slice(0, 7)}\`.`)
}

async function perform(action, head) {
    switch (action.kind) {
        case 'idle':
        case 'wait':
            return
        case 'unlabel':
            await leaveQueue(action.number)
            return
        case 'dequeue':
            await leaveQueue(action.number)
            if (action.failed) await addLabel(action.number, FAILED_LABEL)
            if (action.comment) await comment(action.number, action.comment.body)
            return
        case 'update-branch':
            await updateBranch(action)
            return
        case 'merge':
            await merge(action, head)
            return
        default:
            throw new Error(`unknown action ${action.kind}`)
    }
}

// --- the tick -------------------------------------------------------------

async function tick() {
    heading('tick')

    const defaultBranch = await getDefaultBranch()
    const required = await getRequiredChecks(defaultBranch)
    // Every other failure here ends in a tick that does nothing. This one
    // would end in a tick that merges, because a pull request trivially passes
    // an empty list of required checks. Stop instead.
    if (required.length === 0) {
        log(`ERROR: no required status checks could be resolved for ${defaultBranch}, and no fallback is set.`)
        log('       Refusing to merge anything until MERGE_QUEUE_REQUIRED_CHECKS or the ruleset says what must pass.')
        process.exitCode = 1
        return
    }

    const queue = await getQueue()

    if (queue.length === 0) {
        log('the queue is empty')
        return
    }

    log(`queue (${queue.length}):`)
    for (const [index, entry] of queue.entries()) {
        log(`  ${index + 1}. #${entry.number} ${entry.title} (queued ${entry.labeledAt})`)
    }

    // Labels nobody reads back. A queue that merges without them is working;
    // a queue that refuses to merge because it could not write one is not.
    try {
        await syncPositions(queue)
    } catch (error) {
        log(`WARNING: could not update the queue position labels: ${error.message}`)
    }

    const head = await hydrate(queue[0], defaultBranch, required)

    log(`head of queue: #${head.number} at ${head.headSha}`)
    log(`  queued by @${head.queuedBy ?? 'unknown'} (authorized: ${head.queuedByAuthorized})`)
    log(`  state=${head.state} merged=${head.merged} mergeable=${head.mergeable} behind=${head.behindBy}`)
    log(`  checks=${head.checks.state}`)
    if (head.checks.failed.length > 0) {
        log(`    failed: ${head.checks.failed.map((failure) => `${failure.name} (${failure.detail})`).join(', ')}`)
    }
    if (head.checks.pending.length > 0) log(`    pending: ${head.checks.pending.join(', ')}`)
    if (head.checks.missing.length > 0) log(`    not reported yet: ${head.checks.missing.join(', ')}`)

    const action = decideTick({ queue, head })
    log(`decision -> ${action.kind}: ${action.reason}`)

    await perform(action, head)
}

async function main() {
    if (!environment.MERGE_QUEUE_TOKEN) throw new Error('MERGE_QUEUE_TOKEN is not set')
    if (!owner || !repo) throw new Error('GITHUB_REPOSITORY is not set')

    log(`merge queue: ${owner}/${repo}, triggered by ${eventName}`)

    await ensureLabels()

    if (eventName === 'issue_comment') await handleComment()
    if (eventName === 'pull_request_target' && event.action === 'labeled') {
        // The label was taken back off a moment ago, and the endpoint that
        // lists labelled pull requests does not necessarily know that yet.
        // Ticking now risks acting on the very thing that was just refused.
        if (await handleLabeled()) {
            log('the label was refused; leaving the tick to the next event')
            return
        }
    }
    if (eventName === 'pull_request_target' && event.action === 'synchronize') await handleSynchronize()

    await cleanupClosed()
    await tick()
}

main().catch((error) => {
    console.error(error)
    process.exitCode = 1
})
