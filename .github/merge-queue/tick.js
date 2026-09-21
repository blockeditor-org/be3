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
    LABEL,
    decideComment,
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
async function labeledAt(number) {
    const events = await github.paginate(github.repoPath(`/issues/${number}/events`))
    let when = null
    for (const entry of events) {
        if (entry.event === 'labeled' && entry.label?.name === LABEL) when = entry.created_at
        if (entry.event === 'unlabeled' && entry.label?.name === LABEL) when = null
    }
    return when
}

async function getQueue() {
    const issues = await github.paginate(github.repoPath(`/issues?labels=${encodeURIComponent(LABEL)}&state=open`))
    const entries = []
    for (const issue of issues) {
        if (!issue.pull_request) continue
        entries.push({
            number: issue.number,
            title: issue.title,
            labeledAt: (await labeledAt(issue.number)) ?? issue.updated_at,
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
        checks: summarizeChecks(required, checkRuns, combined?.statuses ?? []),
        botComments: await botCommentKeys(entry.number),
    }
}

// --- writing to the world -------------------------------------------------

async function ensureLabel() {
    const existing = await github.getOrNull(github.repoPath(`/labels/${encodeURIComponent(LABEL)}`))
    if (existing) return
    log(`creating the ${LABEL} label`)
    await github.request('POST', github.repoPath('/labels'), {
        body: { name: LABEL, color: '1f6feb', description: 'Queued to merge into the default branch' },
    })
}

async function addLabel(number) {
    await github.request('POST', github.repoPath(`/issues/${number}/labels`), { body: { labels: [LABEL] } })
}

async function removeLabel(number) {
    try {
        await github.request('DELETE', github.repoPath(`/issues/${number}/labels/${encodeURIComponent(LABEL)}`))
    } catch (error) {
        if (error instanceof ApiError && error.status === 404) return
        throw error
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

    if (action.kind === 'enqueue') await addLabel(issue.number)
    if (action.kind === 'cancel') {
        await removeLabel(issue.number)
        await comment(issue.number, action.body)
    }
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
        log(`#${pullRequest.number} is not queued; nothing to do`)
        return
    }
    // The queue's own update-branch call arrives here as a push by the app.
    // Failing to recognise it would make every update dequeue the thing it
    // just updated, so when the app's own login is unknown, any bot gets the
    // benefit of the doubt rather than none.
    const isOwnPush = botLogin ? sender === botLogin : event.sender?.type === 'Bot'
    if (!botLogin) {
        log('WARNING: MERGE_QUEUE_APP_SLUG is not set, so pushes by this queue cannot be told from other bots.')
    }
    if (isOwnPush) {
        log(`#${pullRequest.number} was updated by ${sender}, which is this queue; keeping it queued`)
        return
    }

    log(`#${pullRequest.number} got new commits from @${sender}; dequeueing`)
    await removeLabel(pullRequest.number)
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
        await removeLabel(issue.number)
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
            await removeLabel(outcome.number)
            if (outcome.comment) await comment(outcome.number, outcome.comment.body)
        }
        return
    }

    log(`merged #${action.number}`)
    await removeLabel(action.number)
    await comment(action.number, `Merged by the merge queue at \`${action.sha.slice(0, 7)}\`.`)
}

async function perform(action, head) {
    switch (action.kind) {
        case 'idle':
        case 'wait':
            return
        case 'unlabel':
            await removeLabel(action.number)
            return
        case 'dequeue':
            await removeLabel(action.number)
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

    const head = await hydrate(queue[0], defaultBranch, required)

    log(`head of queue: #${head.number} at ${head.headSha}`)
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

    await ensureLabel()

    if (eventName === 'issue_comment') await handleComment()
    if (eventName === 'pull_request_target' && event.action === 'synchronize') await handleSynchronize()

    await cleanupClosed()
    await tick()
}

main().catch((error) => {
    console.error(error)
    process.exitCode = 1
})
