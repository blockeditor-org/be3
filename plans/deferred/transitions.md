# Transitions

Depends on: none.

Set aside until the user picks it up.

What a view shows while a write's async work settles, following SolidJS 2.0: a
write that makes async memos run again is held back as a whole until they
answer, so the view moves from one settled state to the next and never shows
the half-loaded one between. Optimistic actions sit on top, so the place that
started the write can show its result at once.

## What exists

`create_async_memo` keeps its last good value while a re-run is in flight and
says so through `pending()`. A view that reads one sees the old answer, then
the new one, with nothing in between; across several async memos it can show
some new answers next to some old ones. `Loading` waits only before its first
reveal (decisions/loading_state_is_ready_and_waits_at_a_boundary.md), so it
plays no part after that.

## Transitions

- `transition(|| { ... })` runs its writes into a fork of the graph: signals
  written inside it hold their new values only for computations run as part of
  the transition, and the async memos those computations reach re-run there.
- The writes land together, and the effects they wake run, once every async
  memo the transition reached has answered (or failed). Until then the view
  keeps showing the committed state, and the transition's `pending()` is true.
- A second transition writing a signal the first wrote joins it (Solid's
  entanglement), so one settled state follows the other. Writes made outside
  any transition land at once, as today, and also apply in the fork.
- A failure inside a transition goes to the nearest `Errored` once it commits,
  like any failure read. Read before it commits, it is held with the
  transition's other answers: the view keeps the committed state and no
  `Errored` shows it yet.
- An `Errored` reset while a transition is open asks its failed async memos
  again outside the transition, as today. A failure the transition read
  under that `Errored` and still holds goes to it at the commit if the
  transition's own re-run fails again; the reset does not drop the held
  failure, since the transition re-runs its async memos itself.

What it needs in beui-reactive: a version of each signal per open
transition, computations that run against a transition's versions, and a
commit that swaps them in at once. That touches every read path, so it is
measured against the bench suite before and after.

## Optimistic actions

- `create_optimistic(source)`: a signal that shows `source` except while an
  action holds it, when it shows what the action wrote. The action is an async
  function (an edit sent to the host); once it settles, the optimistic value
  drops and `source`, by then holding the confirmed value, shows again.
- An action that fails drops its optimistic value too, and its failure goes to
  the nearest `Errored`.
- Block content needs none of this: a `ContentProjection` already applies an
  operation to the visible content at once and reconciles it with the
  replica's answer (guides/blocks/apps.md). This is for host calls that are not
  operations, such as renaming a block or changing access.

## Open questions

- Whether a transition's held writes should hold back the frame's input
  handling for the views that read them, or only what they paint.
- Whether an editor's `<Loaded>` should become a transition boundary after its
  first reveal, so a block switched in place shows the old block until the new
  one is read.
