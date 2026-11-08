# Block-bound agents

Depends on: none.

An agent account that can reach one block and nothing else, for a program
that hosts a block from a machine the person trusts less than their own
devices, such as a terminal host inside a cloud VM
(plans/deferred/boat_terminals.md). Not implemented yet. Set aside with boat's
terminals; the notes below are a review's, to settle before building it.

## What exists

- `create_agent` in `be-host-stack/src/worker.rs` makes an agent from the
  host's Agents menu only: the server makes a passwordless editor account and
  token (`CreateAgent`), the host makes its recovery phrase, puts it in the
  creator's contacts and pins the escrow key for it. The credentials are
  `AgentCredentials` (name, server, organization, token, phrase), which
  `be` reads from `BE3_AGENT_CREDENTIALS`.
- An agent sees only what is shared with it, but it can still do what any
  signed-in member can (the `Rule`s in `be-server/src/client.rs`): make agents
  of its own, make and accept contact links, create blocks at the root and
  upload objects, and make organizations.
- An account may have at most `MAX_AGENTS_PER_CREATOR` (16) agents
  (`be-server/src/store.rs`).
- Sharing a block makes it a scope root sealed to the grantee
  (`Peer::share`), so a grantee of one block gets that block's key alone.

## The design

- **An agent with a `bound_to` block.** The `agents` row names the block; the
  account is otherwise today's agent.
- **The server's limit.** In `admit`, a bound agent passes only calls about
  its block and the blocks under it: keys and objects for them, read, watch
  and publish, and their live sessions. Everything else is refused:
  `CreateAgent`, contacts, `CreateBlock` outside the block, organizations,
  pairing.
- **One live connection.** The server refuses a second connection for a bound
  agent while one is open, so a copy of its credentials can't run beside the
  real one.
- **Lifetime.** Deleting the block, or removing the agent's grant, deletes the
  agent. Bound agents don't count toward the creator's 16. The block's access
  list shows it ("boat VM bx_… (terminal host)"), not the Agents menu.
- **Host calls for apps**, gated by the manifest:
  `create-block-agent(block, name)`, which makes the agent, shares the block
  with it and answers the credentials; `rotate-agent-token(agent)`, which
  issues a new token and revokes the old ones; and `remove-agent(agent)`. The
  app hands the credentials to the machine itself.

## Open before building

- **The allowlist.** Many calls `be terminal-host` makes on connect name no
  block: `OpenOrganization`, `GetKeys`/`PutKeys`/`GetKeyWork`, `PinEscrow`,
  `SessionNew`, `Changes`, `ResealKeys`, `UploadUrls`/`DownloadUrls`
  (`Rule::Organization`/`SignedIn` in `be-server/src/client.rs`). List which a
  bound agent may make, limited to its block where they carry keys or
  objects. Done when the existing `a_terminal_host_*` tests pass run as a bound
  agent, and each refused call has a test.
- **A new connection replaces the old** rather than being refused: a VM that
  stopped without closing its socket leaves a half-open connection, which
  would refuse the reconnect after a resume until the server timed it out. A
  token rotation already revokes copies.
- **A cap of their own.** Not counting toward the creator's 16 leaves them
  unbounded; give bound agents a separate limit per block or per creator.
- **Deleting the block**: moving it to the trash, or collecting it? Restoring
  from the trash would otherwise find the agent gone.
- **The manifest field** that gates the host calls needs a name.
- `create-block-agent` needs Manage on the block and answers the agent id,
  which `rotate-agent-token` and `remove-agent` take.
- **The gateway key.** The credentials an app hands to a machine need the
  `Be3-Gateway-Authorization` key when the server is behind a gateway
  (guides/hosting.md), and no app has it.

## Order of work

1. `bound_to`, the server's limit, the one connection and the lifetime rules,
   with server tests for every refused call.
2. The host calls and their `be-sdk-test` doubles.
3. Update guides/tracker.md (Agents) and guides/terminal.md.

## Not planned here

- Narrower grants than Edit for a bound agent.
