# Declarative dragging: drags as the places they can lead to

Depends on: plans/glide_in_unstyled.md and plans/for_each_handle.md, before step 2;
plans/tentative_gestures.md, before a site previews by holding a tentative gesture.

Every drag in beui becomes a description of where the dragged thing can end up, adapted from
*Declarative Dragging* (Horowitz, Blinn, Heer, UIST 2026; https://declarative-dragging.github.io/).
Each place that takes an item says where it could land and where it would rest there; the board
does the rest: which place the pointer is choosing, the floating item, snapping, the preview,
the animation, keyboard moves, Escape, edge scrolling and showing the drag to peers. It replaces
`Draggable`, `DropTarget`, `Take`, `Target`, `DragSource`, `Reorder`'s hit logic, the dock's tab
and tool bar drag geometry and the hand-written drop targets in apps. Nothing keeps the old API.

## Done means

The final PR deletes `Draggable`, `DropTarget`, `Take`, `Target`, `DragSource`, `ReorderGap` and
`Reorder`'s `hover`/`drop_at`, and replaces the geometry at each site: `bars.rs`'s band logic
(`resolve_bar`, `dock_reach`, `thrown`, `float_near`, `lift_goal`, `step_lift`), the dock's
`resolve`/`resolve_in`/`judged`/`takes`, `reorder.rs`'s `slot_at`/`spans` and gap animation, the
video timeline's `drop_target_at`, and the game board's press state machine. Each step's PR
reports the lines each site had before and has after. A drag that ends up longer than it was is
a sign the spec language is missing something; fix the language, not the site.

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
    as a tentative edit (plans/tentative_gestures.md), so every view of the block follows: a
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
  (step 6).
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
3. **`custom`**, the escape hatch: a function from the point and last frame to a frame. It still
   gets the board's floating, Escape, edge scrolling, presence and inspector. A drag written with
   it marks a gap in the language to come back to.

Continuous drags (sliders, scrollbars, splitters, the colour area) move onto it in step 7, and
only if they are as good there; otherwise they keep `on_drag` and the step says why. Panning moves
the camera, not an item, and stays outside the board.

## Shape

In `beui-types` (`drag.rs`) and unstyled `drag_board.rs`, replacing `Target`, `DragSource` and
`Take`. Payloads and places are typed at the component; the board erases them inside.

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
    Custom(Rc<dyn Fn(DragPoint, &DragFrame) -> DragFrame>),
}

/// One frame of a drag: where the item draws, which candidate a release takes. Also what peers get.
pub struct DragFrame { pub item: Rect, pub chosen: Option<CandidateId>, pub floating: bool }
```

Evaluating a spec is pure, `(spec, DragPoint, last frame) -> DragFrame`, so it is tested without a
core. Stickiness and `Thrown`'s velocity are the only state carried between frames. Styled code
writes builders (`closest(..).when_far(floating())`); reaches and radii come from tokens. `Free`
replaces Dragology's `vary`: every continuous drag in the repo is the pointer minus the grab offset,
clamped or quantised, so there is no optimiser until a drag needs one.

- **The source:** `Interactive`'s `drag: Func<PointerPress, Option<Carry<P>>>`, asked once the
  press passes the threshold (or a finger's long press). `None` lets the press go on as `on_drag`,
  which is how the game board drags pieces and pans with one `Interactive`. `Carry::new(payload)`
  previews the source's own face unless given `.preview(..)`. Being declarative lets the board lift
  the same item from the keyboard, glide it home on Escape, and hear when its node is torn down.
- **A site:** `DropSite<P, D> { accepts, spec: Func<P, DragSpec<D>>, commit: Callback<(P, D)> }`,
  a prop on the container that takes the drop (`<List drop={site}>`), which gives it its node.
  Its handle has `over: Memo<Option<D>>`, the chosen place in it; that is all a site draws from.
- **The board:** `board().carrying::<P>()` and `board().is_carried(&key)`.
- **List slots:** `slots_in(rows: ForEachRef<K>, carried: &K) -> Vec<Candidate<After<K>>>` reads
  the rests from the children's laid-out rects (`Measured`'s content position,
  plans/glide_in_unstyled.md), so a list's layout is written once, by the list. Rows of any height,
  wrapped text included, are exact.

`drive`, `Space`, `Driver`, edge scrolling and drops outside every window stay as they are.

### A reorderable list, written directly

What a list site looks like, and what step 2 measures `Reorder` against:

```rust
#[component]
fn TodoList(todos: Signal<Vec<Todo>>) -> NodeId {
    let rows = ForEachRef::<TodoId>::new();
    let site = DropSite {
        accepts: Func::new(|_: TodoId| true),
        spec: Func::new(move |carried: TodoId| closest(slots_in(rows, &carried)).when_far(floating())),
        commit: Callback::new(move |(carried, After(after))| todos.update(|t| move_after(t, carried, after))),
    };
    let order = create_memo(move || {
        let mut ids: Vec<TodoId> = todos.with(|t| t.iter().map(|t| t.id).collect());
        if let (Some(carried), Some(After(after))) = (board().carrying::<TodoId>(), site.over.get()) {
            move_after(&mut ids, carried, after);
        }
        ids
    });
    view! {
        <List drop={site} spacing=0.0>
            <ForEach keys={order} handle={rows}>|id: TodoId, index: ReadSignal<usize>| {
                let title = create_memo(move || todos.with(|t| t.iter().find(|t| t.id == id).map(|t| t.title.clone())));
                let carried = board().is_carried(&id);
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

1. **The engine.** The types and pure evaluation; `Interactive`'s `drag` on both cores (it reaches
   the board through `beui-types`' `Space`/`Driver`, no new dependency); `DropSite` on `List`; keys
   on a lifted item; publishing frames as presence; tests on the fake `Space` (`drag_board/tests`)
   and plain asserts on spec evaluation. beui-demo pages: the list above, free positioning that
   snaps into slots (`closest(slots).when_far(free(area))`), and a switch dragged between its two
   states. An inspector tool, a dock tab like the others (guides/beui/programs.md, The inspector),
   that draws every candidate, its zone and the chosen one.
2. **Lists and trees.** `slots_in`; `Reorder` rewritten on the primitives, or removed if callers are
   as short without it (decide by the line counts). `styled::Tree`'s `reorder`, database lists and
   kanban (each column a site, nothing kanban-specific), the canvas's layers panel and the files
   pane follow; the files pane's folder rows are `Into` candidates zoned to their middle.
3. **Dock tabs, fluid.** The drop highlight goes; the dock moves aside for a carried tab, the way a
   list opens a gap.
   - **Candidates:** for a lifted tab or pane grip, each `DockState` drop is a candidate: group with
     a pane, split a pane or the whole dock on an edge, insert into a tab bar after a tab, or float.
     It carries the `DockLayout` that `layout_tree` gives for the state after it. Locks leave
     candidates out: a locked place is not reachable, so nothing opens there.
   - **The preview:** the dock draws the chosen candidate's layout, not the state's. A change of
     candidate springs every pane's rect from where it is drawn to where it lies in the new layout,
     keyed by leaf and tab. A pane that only exists after the drop grows out of its edge, and one
     that would close shrinks into its neighbour. `DockState` is untouched until the release.
   - **The carried tab** floats under the pointer, shaped like where it would rest (a tab in a bar,
     or a pane the size of the opening), and glides into that rest on release.
   - **Panels keep their size while the layout moves.** A pane shows its panel at the size it was
     laid out at, clipped to the pane's animated rect, and resizes it only when the drop commits;
     resizing every frame would make every app and Wayland program in a pane lay out and present
     again on each frame (guides/apps/runtime.md, Settling). Hiding the jump at the resize is
     deferred (plans/deferred/dock_drop_resize_blur.md).
   - **Keys and bars:** Alt while dropping keeps the tab floating. Dock bars belonging to a tab move
     with its pane in the preview.

   Expect a different feel, not a copy: replace the drop-highlight tests and paintings with ones
   that show the dock parting for a carried tab.
4. **Dock tool bars.** `bars.rs`'s band geometry becomes
   `closest(band slots, sticky).when_far(free(float)).thrown()`, with the slots read from the bars'
   laid-out rects. Bars beside a slot move aside for the carried bar. Update tests that pin the old
   thresholds.
5. **Apps.**
   - **video timeline:** `TimelineDropTarget` becomes candidates for attach, base slot (named by
     neighbour) and offset; `DropMarker` and `land` go, the board draws from `over`.
   - **infinite canvas:** item moves become `free`.
   - **game editor:** each `Gesture::Drag { from, to }` is a fixed candidate at `to`'s spot;
     `drag` answers `None` off a piece, so the same press pans.
   - **between blocks:** an item dragged into another block of a type that takes it (canvas to
     canvas, list to list) moves on a plain drop and copies with a modifier, through the
     clipboard's typed formats, so a drop is a paste. Without write access to the source it copies.
     The source holds a tentative removal; on drop it commits and the target writes the insert.
   - **settings' display arrangement:** every touching position is a candidate, `when_far(free)`.
   - **bottom sheet:** detents are `closest(detents).thrown()`.
6. **Across apps.** Each app region is an `Into` candidate on the host's board. While the drag is
   over it, the host sends the region the pointer and payload; the app runs the same engine over
   its own candidates, draws its own preview, and reports what it accepts and the rect it chose,
   so the host glides the item there on release. The app protocol carries only points, rects and
   opaque ids (guides/apps/host_services.md). `Editor::drag` and `accept_drag` become this handoff.
   The dock has no app boundary, so it stays one level.
7. **Several carries at once.** Allow more than one carry, one per pointer, when the VR layer's two
   controllers (or touch) need it; steps 1 to 6 key carries by pointer but carry one at a time.
8. **Continuous drags.** Sliders, scrollbars, splitters and the colour area as `free` along an axis,
   quantised. Move each only if it is as good as today.
9. **Remove the rest.** Delete the old API. Rewrite components.md's Drag and drop, docking.md's
   Dragging, Dialogs and tool bars sections, and host_services.md. Record
   decisions/beui_drags_are_reachable_places.md with what was rejected (building speculative
   states, blending view trees, a numeric optimiser, a drag mode that writes before release,
   implicit context arming glides).

## Open questions

- **Moving between sites.** The receiving site's `commit` inserts the item; the source's removal
  is a tentative edit it commits on the drop. Whether `Carry` hears that through a `moved_away`
  callback or the payload carries its own removal is decided in step 2.
- **A carried item far from every site:** does its home keep the gap (no jump when it comes back)
  or close it (today a flicked-out row leaves at once)? Decide in step 2.
- **Rows a `VirtualList` has not built** have no rects. Either `slots_in` uses its estimates, or
  containers answer "where would a child of this size go between each pair" themselves; a
  wrapping grid needs the latter. Decide when a site needs it.
- **Size in a different container.** An item carried into a narrower list keeps its home size until
  it lands; measuring it at the new width means laying out a node that does not exist. Accept a
  one-frame correction unless it shows.
- **Rests at lift vs the reflow.** Choosing against the layout at lift keeps places still under the
  pointer, but rows shift once a gap opens. Pick one convention in step 1 and draw it in the
  inspector.
- **`Between`.** Nothing blends between discrete states today. Add it only if the switch or the
  dock opening in proportion to the pointer (step 3) needs it.
- **A site's content changing mid-drag** (a peer removes a pane): recompute that site's candidates.
