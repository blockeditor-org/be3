# Declarative dragging: drags as the states they can lead to

Depends on: none.

Every drag in beui becomes a description of where the dragged thing can end up, adapted from
*Declarative Dragging* (Horowitz, Blinn, Heer, UIST 2026; https://declarative-dragging.github.io/).
What moves gives the drop states it can reach and, for each, where it would rest; the board
works out everything else: which state the pointer is choosing, the floating item, snapping,
what the user sees before letting go, the animation, keyboard moves and Escape. It replaces
`Draggable`, `DropTarget`, `Take`, `Reorder`'s hit logic, the dock's tab and tool bar drag
geometry and the hand-written drop targets in apps. Nothing keeps the old API.

## The paper, briefly

In the paper's library (Dragology, for React and SVG), an element is draggable when it names
the model states dragging it can reach. When a drag starts, the engine renders each state, reads
where the dragged element lands in each one, and builds the interaction from a small language:

- `closest(states)` snaps to the nearest one;
- `between(states)` blends inside their convex hull;
- `vary(state, param)` drives a number with an optimiser;
- `when_far(a, b)` uses `b` beyond a reach from `a`'s targets;
- `with_floating` makes the item follow the pointer;
- `with_snap_radius` pulls it in near a target, optionally chaining on: entering that state
  without letting go;
- `drop_target(id, state)` and `switch_to_state_and_follow` handle what is not repositional
  (a bin, a palette that creates).

Each frame, the behaviour maps the pointer to a preview and a drop state. Animations come from
blending view trees attribute by attribute (`lerpViews`).

## What beui takes, and what it does differently

- **Placement is computed from data, not by rendering.** beui is retained and reactive, so
  building a component for a state that doesn't exist would build nodes, open scopes and run
  effects in a world that never happens; an effect there may write a block. Here a candidate
  carries its resting rect, computed from data the site already holds:
  - **the dock:** `beui_dock_state::layout_tree` on the state after the drop, a pure function;
  - **a list:** prefix sums of the item sizes it has already measured;
  - **canvas and timeline:** arithmetic on the model.

  No hypothetical node is ever built. The cost is one layout of data per candidate at lift: a
  list's n slots in one O(n) pass, and the dock's few hundred candidates over its handful of
  panes.
- **Preview by applying, animation by `Glide`.** While the chosen candidate holds, the site
  applies its drop state inside a gesture (`begin_gesture`), as editors already write every
  change as it happens (decisions/editors_write_every_change_as_it_happens.md). A release
  commits the gesture, and Escape or a drop nowhere cancels it, which puts back what it wrote.
  The retained nodes glide from their old layout to the new one, as `Reorder` already does.
  Nothing blends view trees. `between` blends only the item's own rect.
- **Sites contribute, not just the source.** A Dragology drag is wholly the source's. In beui a
  tab can land in any pane, a row in any list that accepts it, a block in any app. When a drag
  is lifted, the board asks every registered site that accepts the payload for its spec, and the
  drag's spec is `closest` over the source's own spec and those. A site replaces `DropTarget`.
  Among candidates that coincide, the site painted last wins, as targets do today.
- **Placements are relative to a node.** A candidate's rect is relative to its site's node, and
  each frame the board maps it through `Space::placed`. A list that edge-scrolls during the drag,
  a resized window or a second window moves the candidates without recomputing them. Recomputing
  happens only on chaining, or when the site's content changes underneath (a peer's edit).
- **Keys reach the same states.** Arrow keys on a lifted item move to the nearest candidate in
  that direction, and Enter commits, so every drag can be done from the keyboard: list
  rows (today's Alt+arrow `step`), dock tabs, tool bars, canvas items. A candidate's optional
  `label` is what a screen reader announces.

## Shape

In `beui-types` (`drag.rs`) and unstyled `drag_board.rs`, replacing `Target`, `DragSource` and
`Take`:

```rust
pub struct Candidate {
    pub drop: Rc<dyn Any>,       // handed to the site's apply/commit
    pub rest: Rect,              // where the item rests, relative to `node`
    pub node: NodeRef,
    pub label: Option<String>,
}

pub enum DragSpec {
    Fixed(Candidate),
    Closest { of: Vec<DragSpec>, sticky: f32 },
    Between(Vec<DragSpec>),
    Free { area: NodeRef, drop: Rc<dyn Fn(Rect) -> Rc<dyn Any>> }, // follows the pointer, clamped
    WhenFar { near: Box<DragSpec>, far: Box<DragSpec>, reach: f32 },
    Floating(Box<DragSpec>),
    SnapRadius { spec: Box<DragSpec>, radius: f32, chain: bool },
    Thrown(Box<DragSpec>),       // a fast release continues along its path
    Onto { node: NodeRef, drop: Rc<dyn Any> }, // the escape hatch: a bin, an app region
    SwitchAndFollow { drop: Rc<dyn Any>, follow: NodeRef },
}

/// One frame of a drag: where the item draws, which candidate a release takes, floating or not.
pub struct DragFrame { pub item: Rect, pub chosen: Option<CandidateId>, pub floating: bool }
```

Evaluating a spec is pure, `(spec, DragPoint, last frame) -> DragFrame`, so it is tested without
a core. The stickiness (`sticky`) and `Thrown`'s velocity are the only state carried between
frames. Styled code reaches these through builders (`closest(...).when_far(free(area))`), never
raw pixels: reaches and radii come from tokens. `Free` replaces Dragology's `vary`: every
continuous drag in the repo (canvas items, a floating bar or window, a clip's start) is the
pointer minus the grab offset, clamped or quantised, so there is no optimiser. Add one only when
a drag needs one.

A site is `DropSite { node, accepts, spec(payload) -> DragSpec, apply(drop), commit(drop),
revert() }`. The source is `Lift { payload, own: DragSpec, preview }` on what is pressed.
`drive`, `Space`, `Driver`, edge scrolling and drops outside every window stay as they are.

## Steps

1. **The engine.** The types and pure evaluation, wiring them into the board, keys on a lifted
   item, tests on the fake `Space` (`drag_board/tests`). Also beui-demo pages: a reorderable
   row, free positioning that snaps into slots (`closest(slots).when_far(free)`), and a two-state
   switch dragged between its states. Add an inspector tool, a dock tab like the others
   (guides/beui/programs.md, The inspector), that draws every candidate, its zone and the chosen
   one: the paper's Overlay and Drop Zones visualisers.
2. **Lists and trees.** `Reorder` becomes a site whose candidates are its insertion slots from
   measured sizes (`VirtualList`'s estimates for rows it hasn't measured), with
   `closest(slots).when_far(floating)`. Drop `ReorderGap` hit testing and `hover`/`drop_at`: a
   host's block drag reaches a list as a site like any other. `styled::Tree`'s `reorder`,
   database lists and kanban, and the files pane follow.
3. **Dock tabs.** For a lifted tab or pane grip, each pane, edge and tab bar becomes a candidate
   built from a `DockState` drop (group, split on an edge, insert at a bar index, float), its rect
   from `layout_tree` after that drop. Locks leave candidates out instead of refusing them: a
   locked place simply isn't reachable, and the tab glides home. Alt while dropping keeps
   floating. The highlight that names the grouped tab reads the chosen candidate.
4. **Dock tool bars.** `unstyled/bars.rs`'s band geometry (half-band reach, hold on past it,
   pull halfway, `thrown`, `float_near`) becomes
   `closest(band slots, sticky).when_far(free(float)).thrown()`, with the slots laid out from the
   bars' measured sizes. Whatever the current tests and paintings pin down must still hold.
5. **Apps.**
   - **video timeline:** `TimelineDropTarget` becomes candidates for attach, base index and
     offset;
   - **infinite canvas:** item moves become `free`;
   - **game editor:** each `Gesture::Drag { from, to }` is a fixed candidate at `to`'s spot, so
     the piece snaps and the board highlights from the same data;
   - **be-host's app views:** drags through `Editor::drag`.
6. **Across apps.** The host's board treats each app region as a site. An app answers a drag
   with candidates as plain rects and opaque ids in its frame's coordinates, which keeps the app
   protocol free of beui (guides/apps/host_services.md). `accept_drag`'s yes or no becomes that
   list.
7. **Remove the rest.** Delete `Draggable`, `DropTarget`, `Take`, `Target` and `DragSource`.
   Rewrite components.md's Drag and drop, docking.md's Dragging and Dialogs, tool bars sections,
   and host_services.md. Record the decision (decisions/beui_drags_are_reachable_states.md) with
   what was rejected: speculative builds, view blending, a numeric optimiser.

## Open questions

- Does `Between` earn its place? Nothing in the repo blends between discrete states today.
  Leave it out of step 1 unless the beui-demo switch needs it.
- Writing a chosen state as a preview means peers see a reorder change its order once per
  chosen slot during the drag, not just at the end. That is what the decision on writing
  changes asks for, but for lists it is new. If it reads as noise, a site can preview locally
  and write on commit; decide in step 2.
- What drag a press starts is decided at press time. Should a site be able to change its spec
  mid-drag beyond chaining, for example when a peer's edit removes a pane? Recomputing that
  site's candidates is the simple answer.
