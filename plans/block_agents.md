# Block-bound agents

Depends on: none.

An agent account that can reach one block and nothing else, for a program
that hosts a block from a machine the person trusts less than their own
devices, such as a terminal host inside a cloud VM
(plans/boat_terminals.md). Not implemented yet.

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
  account is otherwise today's agent, with Edit on the block.
- **The server's limit.** In `admit`, a bound agent may make the calls that
  name no block but that `be terminal-host` needs on connect
  (`OpenOrganization`, `GetKeys`/`PutKeys`/`GetKeyWork`, `PinEscrow`,
  `SessionNew`, `Changes`, `ResealKeys`, `UploadUrls`/`DownloadUrls`;
  `Rule::Organization`/`SignedIn` in `be-server/src/client.rs`), each answer
  that carries keys or objects filtered to its block and the blocks under it;
  and the calls about those blocks: read, watch and publish, and their live
  sessions. Everything else is refused: `CreateAgent`, contacts, `CreateBlock`
  outside the block, organizations, pairing.
- **One live connection.** A new connection for a bound agent replaces the
  open one, which the server closes, so a VM that stopped without closing its
  socket reconnects at once after a resume. A token rotation revokes copies.
- **Lifetime.** While the block is in the trash the agent's calls are refused
  and the agent stays, so Restore brings it back. Collecting the block, or
  removing the agent's grant, deletes the agent. The block's access list shows
  it ("boat VM bx_… (terminal host)"), not the Agents menu.
- **A cap of their own.** Bound agents don't count toward the creator's 16
  (`MAX_AGENTS_PER_CREATOR`); a block may have at most 4
  (`MAX_AGENTS_PER_BLOCK`).
- **Host calls for apps**, gated by the manifest's `"block_agents": true`:
  `create-block-agent(block, name)`, which needs Manage on the block, makes the
  agent, shares the block with it and answers the agent id and the
  credentials; `rotate-agent-token(agent)`, which issues a new token and
  revokes the old ones; and `remove-agent(agent)`. The app hands the
  credentials to the machine itself.
- **The gateway key** is not part of the credentials: a machine behind a
  gateway gets it added by hand (plans/boat_terminals.md).

## Order of work

1. `bound_to`, the server's limit, the replacing connection, the cap and the
   lifetime rules. Done: the `a_terminal_host_*` tests pass with the terminal
   host signed in as a bound agent, and a server test per refused call, per
   trash and collect case, and for a second connection replacing the first.
2. The host calls and their `be-sdk-test` doubles.
3. Update guides/tracker.md (Agents) and guides/terminal.md.

## Not planned here

- Narrower grants than Edit for a bound agent.
