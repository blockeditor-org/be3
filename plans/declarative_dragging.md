# Declarative dragging: drags as the places they can lead to

Depends on: plans/host_as_platform_adapter.md, step 4g7 (the compositor in the root, whose router
carries the cross-app drag session).

Every drag in beui becomes a description of where the dragged thing can end up, adapted from
*Declarative Dragging* (Horowitz, Blinn, Heer, UIST 2026; https://declarative-dragging.github.io/).
Each place that takes an item says where it could land and where it would rest there; the board
does the rest: which place the pointer is choosing, the floating item, snapping, the preview,
the animation, keyboard moves, Escape, edge scrolling and showing the drag to peers. Every drag
within an app is built this way; what is left is drags that cross apps.

## Done means

The PR makes the host's block drag the cross-app handoff (`Editor::drag`, `accept_drag`), moves
the files pane's tree onto the board and deletes `Reorder`'s tree-only `hover`/`part`/`shut`/
`resting` with the `visit` and `slot_at` only they use. Every drag within one app is already a
carry on the board (decisions/beui_drags_are_reachable_places.md). `Reorder` stays, built on the
primitives, with its gap spring and `ReorderGap`
(decisions/beui_reorder_stays_over_drop_sites.md). The PR reports the lines each site had before
and has after. A drag that ends up longer than it was is a sign the spec language is missing
something; fix the language, not the site.

## The paper, briefly

In Dragology (React and SVG), an element is draggable when it names the model states dragging it
can reach. At a drag's start the engine renders each state, reads where the element lands in each,
and builds the interaction from a small language: `closest(states)` snaps to the nearest,
`between(states)` blends inside their hull, `vary(state, param)` drives a number with an
optimiser, `when_far(a, b)` uses `b` beyond a reach from `a`, `with_floating` follows the pointer,
`with_snap_radius` pulls in near a target (optionally chaining: entering that state without
letting go), and `drop_target`/`switch_to_state_and_follow` handle what is not repositional.

## What beui does differently

- **Rests come from layout that exists, never from building a state.** Building components for a
  state that does not exist would build nodes, open scopes and run effects, which may write a
  block. A list's rests are read from its children's laid-out rects (`slots_in`, below); the dock
  runs `beui_dock_state::layout_tree` on the state after a drop, a pure function; a canvas or
  timeline computes them from its model. One O(n) pass when the item is lifted.
- **Nothing is written before release** (decisions/tentative_edits_wait_for_release.md). The
  board reports the chosen place (`over`) and ends with a commit or a cancel; how a site previews
  is its own choice of rendering, never a second drag mode:
  - **from `over`:** it draws the chosen place itself (a list's gap, the dock's parted layout, a
    drop caret). For a site whose content is not a block, or where applying would cost too much
    (the dock: every app in a pane would lay out each frame).
  - **in a tentative gesture:** inside the block the drag started in, it applies the chosen place
    as a tentative edit (decisions/tentative_edits_wait_for_release.md), so every view of the block follows: a
    canvas's bound lines, its z-order while a layer is dragged in the sidebar, a builder's layout.
    The edit is sent and saved only on commit; Escape, a crash or a closed instance leave nothing.

  Peers see the drag as it happens either way: the board publishes each frame
  (`{ payload key, chosen place, item rect }`) as presence (`editor.show`, guides/apps/editor.md,
  Presence), and their copy of the site draws the same preview from it.
- **Choosing and resting are separate.** Nearest rest is the default way to choose, but a
  candidate may give a zone instead. A split's rest is a whole half of a pane, far from the edge
  the pointer is at, so the dock chooses by edge bands. Zones and rests are measured against the
  layout at lift, never the one the preview is morphing, so the place being chosen does not move
  away from the pointer choosing it.
- **Places are named by neighbours, not indexes.** A list candidate is `After(Option<K>)`, as the
  storage layer already says it (`ChildChange::Reorder { block, after }`). A peer's insert does not
  move the anchor, and the "index with or without the carried item" off-by-one cannot happen. The
  preview and the commit apply the same `move_after`, so they cannot disagree.
- **Sites contribute, not only the source.** When an item is lifted, every site that accepts its
  payload gives candidates, and the drag is `closest` over the source's own and theirs.
- **Sites nest by handing off.** A candidate may be `Into(region)`: the outer board only chooses
  the region, and the region runs the same engine over its own candidates. This is how nested
  lists and the file tree's "between rows or into a folder" combine, and how drags cross apps
  (step 1).
- **Candidates are relative to a node.** Each frame the board maps them through `Space::placed`,
  so edge scrolling, a resized window or a second window move them without recomputing.
  Recomputing happens only on chaining or when the site's content changes underneath it.
- **One carry per pointer.** Carries are keyed by what carries them: the mouse, each finger, each
  controller, the keyboard. Escape cancels every carry; an item already carried cannot be lifted
  again (a site may turn a second finger on it into its own gesture, a canvas's rotate). Several
  drags in one block join one tentative gesture, committed when the last ends (the decision).
- **Keys reach the same places.** A focused item is lifted with a key, arrow keys walk to the
  nearest candidate in that direction, Enter commits, Escape cancels. A candidate's `label` is
  what a screen reader announces. Every drag becomes doable from the keyboard.

## Scope, in three layers

1. **Starting a drag**, for every pointer drag: the threshold, long press on touch,
   `touch_drags`, `capture_presses`, and choosing between dragging and something else. It lives in
   `Interactive`'s `drag` prop, so no component keeps its own press state machine.
2. **The spec language**, for moves into places (lists, dock, timeline, game pieces, across apps)
   and drags that snap between rests (sheet detents, display arrangement, a switch).
3. **`custom`**, for a site whose places are a function of the point (the video timeline, where a
   clip lands at the frame under the pointer): `place` maps a point in the site's node to the
   drop there, or none, and the site draws where the item would land from its `over`. It still
   gets the board's floating, Escape, edge scrolling, presence and inspector, and the board tells
   the site again as the drop changes under the pointer.

Continuous drags (sliders, scrollbars, splitters, the colour area and wheel) are on it too,
each as good there as on `on_drag` and with Escape besides. Panning moves the camera, not an item, and
stays outside the board.

## Shape

In `beui-types` (`drag.rs`) and unstyled `drag_board.rs`. Payloads and places are typed at the component; the board erases them inside.

```rust
pub struct Candidate<D> {
    pub drop: D,
    pub rest: Rect,          // where the item would rest, relative to `node`
    pub zone: Option<Rect>,  // where the pointer chooses it, if not by nearest `rest`
    pub node: NodeRef,
    pub label: Option<String>,
}

pub enum DragSpec<D> {
    Fixed(Candidate<D>),
    Closest { of: Vec<DragSpec<D>>, sticky: f32 },
    Free { area: NodeRef, drop: Rc<dyn Fn(Rect) -> D> },   // the pointer minus the grab, clamped or quantised
    WhenFar { near: Box<DragSpec<D>>, far: Box<DragSpec<D>>, reach: f32 },
    Floating,                                            // carried, choosing nothing here
    SnapRadius { spec: Box<DragSpec<D>>, radius: f32, chain: bool },
    Thrown(Box<DragSpec<D>>),                             // a fast release continues along its path
    Into { region: NodeRef, drop: D },                    // a nested site chooses from here on
    Custom { node: NodeRef, place: Rc<dyn Fn(Pos2) -> Option<D>> }, // places as a function of the point
}

/// One frame of a drag: where the item draws, which candidate a release takes. Also what peers get.
pub struct DragFrame { pub item: Rect, pub chosen: Option<CandidateId>, pub floating: bool }
```

Evaluating a spec is pure, `(spec, DragPoint, last frame) -> DragFrame`, so it is tested without a
core. Stickiness and `Thrown`'s velocity are the only state carried between frames. The unstyled
engine takes reaches, radii and throw speeds as values, with defaults: they are behaviour, like
the drag threshold (decisions/beui_unstyled_layer_has_no_appearance_defaults.md), and the builders (`closest(..).when_far(floating())`)
use those defaults unless given others. `Free`
replaces Dragology's `vary`: every continuous drag in the repo is the pointer minus the grab offset,
clamped or quantised, so there is no optimiser until a drag needs one.

- **The source:** `Interactive`'s `drag: Func<PointerPress, Option<Carry<P>>>`, asked once the
  press passes the threshold (or a finger's long press). `None` lets the press go on as `on_drag`,
  which is how the game board drags pieces and pans with one `Interactive`. `Carry::new(payload)`
  previews the source's own face unless given `.preview(..)`. Being declarative lets the board lift
  the same item from the keyboard, glide it home on Escape, and hear when its node is torn down.
- **A site:** `use_drop_site(accepts: Func<P, bool>, spec: Func<P, DragSpec<D>>, commit:
  Callback<(P, D)>)`, a hook in the component that takes the drop, a site for as long as its scope
  lives. It is a hook, not a `drop` prop on containers: a
  place names its own node through its `Candidate`, so a site needs no node, and no container
  grows a prop. Its handle has `over: ReadSignal<Option<D>>`, the chosen place in it; that is all a
  site draws from.
- **The board:** `carrying::<P>()` and `is_carried(key)`.
- **List slots:** `slots_in(list: NodeRef, rows: ForEachRef<K, NodeId>, carried: &K, direction) -> Vec<Candidate<After<K>>>` reads
  the rests from the rows' laid-out rects (`measure`, with the scroll around them taken out by
  `measure::scrolled_by`, as `Glide` does), so a list's layout is written once, by the list. Rows
  of any height, wrapped text included, are exact; a row of several children rests where its run
  does.

What stays below unstyled, and what changes there:

- **Stays in core:** turning pointer input into presses and drags (thresholds, `touch_drags`, long
  presses, re-sending a held drag each frame for edge scrolling), `Space` on `Rects`, the
  `Document::drags` hook, the built-in Escape, `drive`, and the HTML core's window listeners while
  a drag is carried (`beui-core-html` `drags.rs`). Drops outside every window stay as they are.
- **`Driver` becomes per pointer:** `track(pointer, point)`, `release(pointer, ..)`, so the core
  drives one carry per pointer.
- **`Interactive`'s `drag`** lives on the `beui-view` component, not the core node: at the first
  drag the core already reports, the component calls `drag` and hands the result to the
  registered `Driver`. The core gains no logic.
- **Leaves `beui-types`:** `Target`, `DragSource` and their callback aliases are deleted;
  `EdgeScroller` and `EDGE_BAND`, edge-scroll policy used only by the board and `Scroll` (and
  re-exported by `beui_view::drag`), move to unstyled, the band staying a behaviour
  default (decisions/beui_unstyled_layer_has_no_appearance_defaults.md).

### A reorderable list, written directly

What a list site looks like, and what `Reorder` was measured against:

```rust
#[component]
fn TodoList(todos: Signal<Vec<Todo>>) -> NodeId {
    let rows = ForEachRef::<TodoId, NodeId>::new();
    let list = NodeRef::new();
    let site = use_drop_site(
        Func::new(|_: TodoId| true),
        Func::new(move |carried: TodoId| closest(slots_in(list, rows, &carried, Direction::Vertical)).when_far(floating())),
        Callback::new(move |(carried, After(after))| todos.update(|t| move_after(t, carried, after))),
    );
    let order = create_memo(move || {
        let mut ids: Vec<TodoId> = todos.with(|t| t.iter().map(|t| t.id).collect());
        if let (Some(carried), Some(After(after))) = (carrying::<TodoId>(), site.over.get()) {
            move_after(&mut ids, carried, after);
        }
        ids
    });
    view! {
        <List @node_ref=&list spacing=0.0>
            <ForEach value={order} handle={rows}>|id: TodoId, index: ReadSignal<usize>| {
                let title = create_memo(move || todos.with(|t| t.iter().find(|t| t.id == id).map(|t| t.title.clone())));
                let carried = is_carried(id);
                view! {
                    <Glide on={index}>
                        <Interactive drag={move |_| Some(Carry::new(id))}>
                            <Frame padding=Space::Sm opacity={carried.map(|c| if c { 0.0 } else { 1.0 })}>
                                <Text wrap=true>{title}</Text>
                            </Frame>
                        </Interactive>
                    </Glide>
                }
            }</ForEach>
        </List>
    }
}
```

The carried row's key moves to the chosen place and is never removed, so its component lives
through the drag and Escape rebuilds nothing; drawn invisible, it is the gap, at its real size.

## Steps

1. **Across apps.** plans/host_as_platform_adapter.md owns the cross-app session (Input, "Drag
   and drop"): the source starts it with its payload, and the host routes drag-over events to the
   surface under the pointer and the drop to the one under the release, by its forwarding router.
   This step owns what those events carry and what the surface does with them. Each app region is
   an `Into` candidate on the host's board. A drag-over carries the pointer and the payload; the
   app runs the same engine over its own candidates, draws its own preview, and answers what it
   accepts and the rect it chose, so the host glides the item there on release. The events carry
   only points, rects and opaque ids (guides/apps/host_services.md). A site may name an
   optional node, which `Into` nesting and the inspector want.
   - **`Editor::drag` and `accept_drag` as the handoff.** Today the host's block drag is on the
     host's board: an app's `Editor::drag` lifts a `Carry` of the block at the pointer pressing
     in it (`PointerSample::pointer`), and each view holding an editor's content is a `custom`
     site that only forwards points (`drag_over_view`). It becomes the `Into` candidate above,
     whose app runs the engine over its own places and answers the rect it chose, so the host
     glides the item there. With two pointers pressing, the cross-app session should name the
     one that pressed in the sending view rather than the first.
   - **The files pane's tree.** Its row drag is the host's block drag, which leaves the app
     for other editors, so it waits for this step: `styled::Tree`'s `reorder`
     becomes a drop site whose folder rows are `Into` candidates zoned to their middle, and
     `Reorder`'s tree-only `hover`, `part`, `shut` and `resting` go, with the `visit` and
     `slot_at` only they use. The dock has no app boundary, so it stays one level.
   - **Between blocks.** An item dragged into another block of a type that takes it (canvas to
     canvas, list to list) moves on a plain drop and copies with a modifier, through the
     clipboard's typed formats, so a drop is a paste. Without write access to the source it
     copies. The source holds a tentative removal. On drop the target writes the insert first and
     the source then commits its removal, so a crash between the two leaves a copy, never nothing.
     It waits for this step, not the clipboard: the typed formats are there (`Clip`, `ClipFormat`,
     the canvas copying its items as its own format, refs translated wherever a clip crosses), but
     two blocks are two editor instances, a child block placed in a canvas included, and the only
     drag between instances today is the host's block drag. That carries one block
     (`ShellMessage::DragBlock`, `BlockDrag`), which an app can start but not with a clip of its
     items, and nothing tells the source what the target did with the drop, which the
     insert-then-remove order needs. Both are what this step's drag-over and drop events carry:
     the payload as a clip, and the target's answer, which the host passes back to the source as
     its commit or put-down.

## Open questions

- **Moving between sites** (decided: decisions/beui_a_move_between_drop_sites_is_the_apps.md).
  The receiving site's `commit` gets the payload and the app does the move: it removes the item
  from its home and inserts it at the chosen place in one edit, as it would for any other command.
  `Carry` has no `moved_away` callback. Within one tree, a row could move without being rebuilt
  by being a `Kept` under its key inside one `KeepAlive` around both lists, which moves a subtree
  between parents (guides/beui/core.md); decide when a caller rebuilding rows is too slow.
- **A carried item far from every site** (decided): a `Reorder` row that another
  site takes collapses as its slot closes, and its slot opens again if it is let go where
  nothing takes it; a row nothing else takes stays on its own list's places.
- **A site that comes or changes mid-carry.** Specs, and what decides them (a `Reorder`
  asking `Board::takers` whether anything else takes its row), are read at the lift: a site
  mounted mid-carry gains no places until the next one. Decide when a caller needs it.
- **Rows a `VirtualList` has not built** have no rects. Either `slots_in` uses its estimates, or
  containers answer "where would a child of this size go between each pair" themselves; a
  wrapping grid needs the latter. Decide when a site needs it.
- **Size in a different container.** An item carried into a narrower list keeps its home size until
  it lands; measuring it at the new width means laying out a node that does not exist. Accept a
  one-frame correction unless it shows.
- **`Between`.** Nothing blends between discrete states today. Add it only if a drag needs it.
- **A site's content changing mid-drag** (a peer removes a pane): recompute that site's candidates.
