import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import {
    LABEL,
    classifyMergeFailure,
    decideComment,
    decideTick,
    hasWriteAccess,
    markerKeyOf,
    mergeFailureAction,
    orderQueue,
    parseCommand,
    summarizeChecks,
} from './decide.js'

const SHA = 'a'.repeat(40)
const REQUIRED = ['CI OK']

function green(name = 'CI OK') {
    return [{ id: 1, name, status: 'completed', conclusion: 'success', html_url: 'https://example/1' }]
}

function head(overrides = {}) {
    return {
        number: 7,
        title: 'a change',
        state: 'open',
        merged: false,
        headSha: SHA,
        baseRef: 'main',
        defaultBranch: 'main',
        mergeable: true,
        behindBy: 0,
        checks: summarizeChecks(REQUIRED, green()),
        botComments: [],
        ...overrides,
    }
}

function state(overrides = {}, queueLength = 1) {
    const theHead = head(overrides)
    const queue = Array.from({ length: queueLength }, (_, index) => ({
        number: index === 0 ? theHead.number : 100 + index,
        title: 'queued',
        labeledAt: `2026-09-2${index + 1}T00:00:00Z`,
    }))
    return { queue, head: theHead }
}

describe('parseCommand', () => {
    it('reads /merge and /merge cancel on their own line', () => {
        assert.deepEqual(parseCommand('/merge'), { kind: 'merge' })
        assert.deepEqual(parseCommand('/merge cancel'), { kind: 'cancel' })
        assert.deepEqual(parseCommand('/merge CANCEL'), { kind: 'cancel' })
        assert.deepEqual(parseCommand('looks good\n\n/merge\n'), { kind: 'merge' })
    })

    it('ignores a command that is only being talked about', () => {
        assert.equal(parseCommand('you can say /merge to queue it'), null)
        assert.equal(parseCommand('> /merge'), null)
        assert.equal(parseCommand('`/merge`'), null)
        assert.equal(parseCommand(''), null)
        assert.equal(parseCommand(undefined), null)
    })

    it('reports an unknown subcommand rather than guessing', () => {
        assert.deepEqual(parseCommand('/merge now'), { kind: 'unknown', argument: 'now' })
    })
})

describe('decideComment', () => {
    const base = {
        command: { kind: 'merge' },
        permission: 'write',
        isPullRequest: true,
        prState: 'open',
        hasLabel: false,
        commenter: 'dev',
    }

    it('queues a pull request for someone with write access', () => {
        assert.equal(decideComment(base).kind, 'enqueue')
        assert.equal(decideComment({ ...base, permission: 'admin' }).kind, 'enqueue')
        assert.equal(decideComment({ ...base, permission: 'maintain' }).kind, 'enqueue')
    })

    it('refuses an unauthorized commenter and says why', () => {
        for (const permission of ['read', 'none', 'triage', undefined]) {
            const action = decideComment({ ...base, permission })
            assert.equal(action.kind, 'reply', `permission ${permission} should be refused`)
            assert.match(action.body, /write access/)
            assert.match(action.body, /@dev/)
        }
    })

    it('does nothing for a comment on an issue', () => {
        assert.equal(decideComment({ ...base, isPullRequest: false }).kind, 'ignore')
    })

    it('does nothing when there is no command', () => {
        assert.equal(decideComment({ ...base, command: null }).kind, 'ignore')
    })

    it('cancels only what is queued', () => {
        const cancel = { ...base, command: { kind: 'cancel' }, hasLabel: true }
        assert.equal(decideComment(cancel).kind, 'cancel')
        assert.equal(decideComment({ ...cancel, hasLabel: false }).kind, 'acknowledge')
    })

    it('does not queue twice', () => {
        assert.equal(decideComment({ ...base, hasLabel: true }).kind, 'acknowledge')
    })

    it('refuses to queue a closed pull request', () => {
        const action = decideComment({ ...base, prState: 'closed' })
        assert.equal(action.kind, 'reply')
        assert.match(action.body, /already closed/)
    })

    it('checks permission before anything else it would act on', () => {
        const action = decideComment({ ...base, permission: 'read', hasLabel: true })
        assert.equal(action.kind, 'reply')
    })
})

describe('hasWriteAccess', () => {
    it('accepts write and above only', () => {
        assert.ok(hasWriteAccess('admin'))
        assert.ok(hasWriteAccess('maintain'))
        assert.ok(hasWriteAccess('write'))
        assert.ok(!hasWriteAccess('triage'))
        assert.ok(!hasWriteAccess('read'))
        assert.ok(!hasWriteAccess('none'))
        assert.ok(!hasWriteAccess(undefined))
    })
})

describe('summarizeChecks', () => {
    it('is green when every required check passed', () => {
        assert.equal(summarizeChecks(REQUIRED, green()).state, 'success')
    })

    it('treats a neutral conclusion as a pass', () => {
        const runs = [{ id: 1, name: 'CI OK', status: 'completed', conclusion: 'neutral' }]
        assert.equal(summarizeChecks(REQUIRED, runs).state, 'success')
    })

    it('refuses to count a skipped check as a pass', () => {
        const runs = [{ id: 1, name: 'CI OK', status: 'completed', conclusion: 'skipped' }]
        const result = summarizeChecks(REQUIRED, runs)
        assert.equal(result.state, 'failure')
        assert.match(result.failed[0].detail, /skipped/)
    })

    it('is pending while a required check is still running', () => {
        const runs = [{ id: 1, name: 'CI OK', status: 'in_progress', conclusion: null }]
        const result = summarizeChecks(REQUIRED, runs)
        assert.equal(result.state, 'pending')
        assert.deepEqual(result.pending, ['CI OK'])
    })

    it('is pending while a required check has not reported at all', () => {
        const result = summarizeChecks(REQUIRED, [])
        assert.equal(result.state, 'pending')
        assert.deepEqual(result.missing, ['CI OK'])
    })

    it('fails on a failed check and keeps its link', () => {
        const runs = [
            { id: 1, name: 'CI OK', status: 'completed', conclusion: 'failure', html_url: 'https://example/run' },
        ]
        const result = summarizeChecks(REQUIRED, runs)
        assert.equal(result.state, 'failure')
        assert.equal(result.failed[0].url, 'https://example/run')
    })

    it('takes the newest run when a check was re-run', () => {
        const runs = [
            { id: 1, name: 'CI OK', status: 'completed', conclusion: 'failure' },
            { id: 2, name: 'CI OK', status: 'completed', conclusion: 'success' },
        ]
        assert.equal(summarizeChecks(REQUIRED, runs).state, 'success')
    })

    it('ignores checks that are not required', () => {
        const runs = [...green(), { id: 9, name: 'optional', status: 'completed', conclusion: 'failure' }]
        assert.equal(summarizeChecks(REQUIRED, runs).state, 'success')
    })

    it('reads a commit status when there is no check run by that name', () => {
        const statuses = [{ context: 'CI OK', state: 'success', updated_at: '2026-09-21T00:00:00Z' }]
        assert.equal(summarizeChecks(REQUIRED, [], statuses).state, 'success')
        assert.equal(summarizeChecks(REQUIRED, [], [{ context: 'CI OK', state: 'failure' }]).state, 'failure')
    })

    it('has nothing to fail when nothing is required', () => {
        assert.equal(summarizeChecks([], []).state, 'success')
    })
})

describe('orderQueue', () => {
    it('is first in, first out by when the label was applied', () => {
        const ordered = orderQueue([
            { number: 3, labeledAt: '2026-09-21T12:00:00Z' },
            { number: 1, labeledAt: '2026-09-21T10:00:00Z' },
            { number: 2, labeledAt: '2026-09-21T11:00:00Z' },
        ])
        assert.deepEqual(
            ordered.map((entry) => entry.number),
            [1, 2, 3],
        )
    })

    it('breaks a tie by pull request number so the order is stable', () => {
        const at = '2026-09-21T10:00:00Z'
        const ordered = orderQueue([
            { number: 9, labeledAt: at },
            { number: 4, labeledAt: at },
        ])
        assert.deepEqual(
            ordered.map((entry) => entry.number),
            [4, 9],
        )
    })
})

describe('decideTick', () => {
    it('does nothing with an empty queue', () => {
        assert.equal(decideTick({ queue: [], head: null }).kind, 'idle')
    })

    it('merges a pull request that is up to date and green', () => {
        const action = decideTick(state())
        assert.equal(action.kind, 'merge')
        assert.equal(action.number, 7)
        assert.equal(action.sha, SHA)
    })

    it('updates a branch that is behind, and does not look at its checks', () => {
        const action = decideTick(
            state({ behindBy: 3, checks: summarizeChecks(REQUIRED, [{ id: 1, name: 'CI OK', status: 'completed', conclusion: 'failure' }]) }),
        )
        assert.equal(action.kind, 'update-branch')
        assert.equal(action.expectedHeadSha, SHA)
        assert.match(action.comment.body, /3 commit/)
    })

    it('waits while required checks are still running', () => {
        const action = decideTick(state({ checks: summarizeChecks(REQUIRED, []) }))
        assert.equal(action.kind, 'wait')
        assert.match(action.reason, /CI OK/)
    })

    it('dequeues on a failed required check and links it', () => {
        const runs = [
            { id: 1, name: 'CI OK', status: 'completed', conclusion: 'failure', html_url: 'https://example/run' },
        ]
        const action = decideTick(state({ checks: summarizeChecks(REQUIRED, runs) }))
        assert.equal(action.kind, 'dequeue')
        assert.match(action.comment.body, /https:\/\/example\/run/)
    })

    it('dequeues on conflicts with the base branch', () => {
        const action = decideTick(state({ mergeable: false }))
        assert.equal(action.kind, 'dequeue')
        assert.match(action.comment.body, /conflicts/)
    })

    it('waits rather than merging when the base comparison could not be read', () => {
        for (const behindBy of [null, undefined]) {
            const action = decideTick(state({ behindBy }))
            assert.equal(action.kind, 'wait', `behindBy ${behindBy} must not merge`)
            assert.match(action.reason, /could not compare/)
        }
    })

    it('waits rather than guessing while mergeability is unknown', () => {
        assert.equal(decideTick(state({ mergeable: null })).kind, 'wait')
        assert.equal(decideTick(state({ mergeable: undefined })).kind, 'wait')
    })

    it('unlabels a closed pull request without commenting', () => {
        const action = decideTick(state({ state: 'closed' }))
        assert.equal(action.kind, 'unlabel')
        assert.equal(action.comment, undefined)
    })

    it('unlabels a pull request that was merged elsewhere', () => {
        const action = decideTick(state({ state: 'closed', merged: true }))
        assert.equal(action.kind, 'unlabel')
        assert.match(action.reason, /merged/)
    })

    it('dequeues a pull request aimed at another branch', () => {
        const action = decideTick(state({ baseRef: 'release' }))
        assert.equal(action.kind, 'dequeue')
        assert.match(action.comment.body, /release/)
    })

    it('acts only on the head when several are queued', () => {
        const several = state({}, 3)
        assert.equal(several.queue.length, 3)
        const action = decideTick(several)
        assert.equal(action.kind, 'merge')
        assert.equal(action.number, several.queue[0].number)
    })

    it('says how many are behind the head while it waits', () => {
        const several = state({ checks: summarizeChecks(REQUIRED, []) }, 3)
        assert.match(decideTick(several).reason, /2 behind it/)
    })

    it('says a thing once, however many ticks see the same state', () => {
        const first = decideTick(state({ behindBy: 1 }))
        assert.ok(first.comment)
        const key = markerKeyOf(first.comment.body)
        assert.equal(key, first.comment.key)

        const second = decideTick(state({ behindBy: 1, botComments: [key] }))
        assert.equal(second.kind, 'update-branch')
        assert.equal(second.comment, null)
    })

    it('says it again once the head has moved on', () => {
        const stale = decideTick(state({ behindBy: 1 })).comment.key
        const moved = decideTick(state({ behindBy: 1, headSha: 'b'.repeat(40), botComments: [stale] }))
        assert.ok(moved.comment)
    })
})

describe('classifyMergeFailure', () => {
    it('treats a lost race as transient, because that is the design working', () => {
        assert.equal(classifyMergeFailure(409, 'Head branch was modified. Review and try merging again.'), 'transient')
        assert.equal(classifyMergeFailure(409, 'Base branch was modified. Review and try merging again.'), 'transient')
        assert.equal(classifyMergeFailure(405, 'Pull Request is not mergeable'), 'transient')
        assert.equal(classifyMergeFailure(422, 'Head branch is out of date'), 'transient')
        assert.equal(classifyMergeFailure(502, 'Server Error'), 'transient')
    })

    it('treats a refusal the next tick cannot fix as fatal', () => {
        assert.equal(
            classifyMergeFailure(405, 'At least 1 approving review is required by reviewers with write access.'),
            'fatal',
        )
        assert.equal(classifyMergeFailure(403, 'Resource not accessible by integration'), 'fatal')
        assert.equal(classifyMergeFailure(422, 'Validation Failed'), 'fatal')
    })
})

describe('mergeFailureAction', () => {
    it('waits out a race without touching the queue', () => {
        const action = mergeFailureAction(7, SHA, 409, 'Head branch was modified. Review and try merging again.')
        assert.equal(action.kind, 'wait')
        assert.match(action.reason, /re-evaluate/)
    })

    it('dequeues on a fatal refusal and repeats what GitHub said', () => {
        const message = 'At least 1 approving review is required by reviewers with write access.'
        const action = mergeFailureAction(7, SHA, 405, message, [])
        assert.equal(action.kind, 'dequeue')
        assert.match(action.comment.body, /approving review is required/)
    })

    it('does not repeat a dequeue comment it already left', () => {
        const message = 'At least 1 approving review is required by reviewers with write access.'
        const first = mergeFailureAction(7, SHA, 405, message, [])
        const again = mergeFailureAction(7, SHA, 405, message, [first.comment.key])
        assert.equal(again.kind, 'dequeue')
        assert.equal(again.comment, null)
    })
})

describe('LABEL', () => {
    it('is the name the workflow and the ruleset docs use', () => {
        assert.equal(LABEL, 'merge-queue')
    })
})
