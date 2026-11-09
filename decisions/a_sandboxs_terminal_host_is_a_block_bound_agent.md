# A sandbox's terminal host is a block-bound agent

A terminal host in a machine trusted less than the person's devices, such as a boat.dev sandbox
(guides/terminal.md, A terminal host in a boat.dev sandbox), signs in as an agent bound to its
one terminal block (`create-block-agent`, guides/tracker.md, A block's agent), made by the app
that installs it. The app keeps the agent's credentials in the person's owner-only settings,
writes them into the machine's memory (`/run`), rotates the token after every resume and
removes the agent on Disconnect. The terminal block is made in the person's Unfiled, so the
agent reaches that block and nothing else, and the host lets an app make an agent for a block it
made that still sits alone in the person's Unfiled, as well as for its own block and those below
it. `CREATED_BY` alone is not enough: anyone who may edit a block can set it, and it stays on a
block wherever it moves.

## Why

- Someone who can read the machine's files can take any key on it. An agent bound to one block
  makes the terminal's credential worth no more than the machine: it reaches no other block, no
  contacts, no access lists, and its creator removes it at once.
- The terminal block works unchanged on every platform: the host is `be terminal-host`, as on
  any machine, and viewers only render the document.
- A block in Unfiled needs no Edit on the boat block, so a viewer who may only read it still
  connects a terminal of their own; an agent bound to the boat block would reach every other
  viewer's terminal below it.

## Rejected

- The person's own account, or an ordinary agent, in the machine: it reaches everything the
  person or agent can, and a snapshot or fork of the machine carries it.
- The terminal block under the boat block: making it needs Edit there, and an agent for one
  terminal would be bound beside every other.
- The host running the shell over ssh or the commands API itself: the terminal would work only
  while a be-host that can reach boat.dev is open, and only on platforms that can.
