// A small REST client, because the queue's whole job is a dozen calls and a
// dependency here would be a dependency in the thing that presses merge.

const API = 'https://api.github.com'

export class ApiError extends Error {
    constructor(status, message, body) {
        super(`${status}: ${message}`)
        this.status = status
        this.apiMessage = message
        this.body = body
    }
}

export class GitHub {
    constructor({ token, owner, repo, log = console.log }) {
        this.token = token
        this.owner = owner
        this.repo = repo
        this.log = log
    }

    async request(method, path, { body, accept } = {}) {
        const url = path.startsWith('http') ? path : `${API}${path}`
        const response = await fetch(url, {
            method,
            headers: {
                authorization: `Bearer ${this.token}`,
                accept: accept ?? 'application/vnd.github+json',
                'x-github-api-version': '2022-11-28',
                'user-agent': 'be3-merge-queue',
                ...(body ? { 'content-type': 'application/json' } : {}),
            },
            body: body ? JSON.stringify(body) : undefined,
        })

        const text = await response.text()
        let parsed = null
        if (text) {
            try {
                parsed = JSON.parse(text)
            } catch {
                parsed = text
            }
        }

        if (!response.ok) {
            const message = parsed?.message ?? response.statusText ?? 'request failed'
            throw new ApiError(response.status, message, parsed)
        }

        return { data: parsed, link: response.headers.get('link') }
    }

    async get(path) {
        const { data } = await this.request('GET', path)
        return data
    }

    // Returns null rather than throwing for the statuses that mean "there is no
    // such thing", which for several of these endpoints is an answer and not a
    // problem: a repository with no ruleset, a label that was never created.
    async getOrNull(path, statuses = [404]) {
        try {
            return await this.get(path)
        } catch (error) {
            if (error instanceof ApiError && statuses.includes(error.status)) return null
            throw error
        }
    }

    async paginate(path) {
        const items = []
        let next = path.includes('?') ? `${path}&per_page=100` : `${path}?per_page=100`
        while (next) {
            const { data, link } = await this.request('GET', next)
            const page = Array.isArray(data) ? data : (data?.check_runs ?? [])
            items.push(...page)
            const match = link && /<([^>]+)>;\s*rel="next"/.exec(link)
            next = match ? match[1] : null
        }
        return items
    }

    repoPath(suffix) {
        return `/repos/${this.owner}/${this.repo}${suffix}`
    }
}
