# Declarative dragging: drags as the places they can lead to

Depends on: plans/host_as_platform_adapter.md, step 4f (the forwarding router, built in step 3, routing the
host's input over the layer list), before step 6.

Every drag in beui becomes a description of where the dragged thing can end up, adapted from
*Declarative Dragging* (Horowitz, Blinn, Heer, UIST 2026; https://declarative-dragging.github.io/).
Each place that takes an item says where it could land and where it would rest there; the board
does the rest: which place the pointer is choosing, the floating item, snapping, the preview,
the animation, keyboard moves, Escape, edge scrolling and showing the drag to peers. It replaces
`Draggable`, `DropTarget`, `Target`, `DragSource`, `Reorder`'s own hit logic, the dock's tab
and tool bar drag geometry and the hand-written drop targets in apps. Nothing keeps the old API.

## Done means

The final PR deletes `Draggable`, `DropTarget`, `Target`, `DragSource`, and `Reorder`'s tree-only `hover`/`part`/`shut`/`resting` with the `visit` and
`slot_at` only they use (step 6), and replaces the geometry at each site: `bars.rs`'s band logic
(`resolve_bar`, `dock_reach`, `thrown`, `float_near`, `lift_goal`, `step_lift`; gone in step 4), the dock's
`resolve`/`resolve_in` (gone in step 3b; its `judged` and `takes` in step 3a), and the game board's press state machine. The video
timeline's `drop_target_at` stays as its `custom` site's `place`. `Reorder` stays, built on the primitives, with its gap spring and
`ReorderGap` (decisions/beui_reorder_stays_over_drop_sites.md). Each step's PR
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
3. **`custom`**, for a site whose places are a function of the point (the video timeline, where a
   clip lands at the frame under the pointer): `place` maps a point in the site's node to the
   drop there, or none, and the site draws where the item would land from its `over`. It still
   gets the board's floating, Escape, edge scrolling, presence and inspector, and the board tells
   the site again as the drop changes under the pointer.

Continuous drags (sliders, scrollbars, splitters, the colour area) move onto it in step 7, and
only if they are as good there; otherwise they keep `on_drag` and the step says why. Panning moves
the camera, not an item, and stays outside the board.

## Shape

In `beui-types` (`drag.rs`) and unstyled `drag_board.rs`, replacing `Target`, `DragSource` and
`Take` (gone in step 3a). Payloads and places are typed at the component; the board erases them inside.

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
  lives. It is a hook, not a `drop` prop on containers (provisional, until the step 1 review): a
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

What a list site looks like, and what step 2 measures `Reorder` against:

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

1. **The engine**, in slices, with the old API beside it until step 9:
   - **1a (done):** the types and pure evaluation, with plain asserts on spec evaluation.
   - **1b (done):** drop sites, the board's carry choosing among them, `carrying`/`is_carried`,
     `slots_in`, and `Interactive`'s `drag` on both cores (it reaches the board through
     `beui-types`' `Space`/`Driver`, no new dependency); tests on the fake `Space` and a
     beui-tests screen both cores drive.
   - **1c (done):** the floating preview (`Carry::preview`, and the source's own face by default), a
     `Driver` per pointer, keys on a lifted item, and publishing frames as presence. A source
     whose scope is dropped mid-carry puts the carry down, dropping nothing. The default face is
     the source drawn displaced and raised, which a scroll around it still clips: a carry that
     leaves its scroll gives `.preview`; capturing the face as a texture is in the backlog.
     Presence in 1c is only the hook: the board's tracked `frame()`, which a host publishes
     from an effect; peers drawing a remote carry wait for step 5, once a site has a stable id
     to name places by. Keys: Ctrl+Shift and an arrow move a focused source one place; Move in the palette,
     or Space on a source that does not activate, lifts it; the arrows step between its places
     (`step_toward`), Space or Enter drops, Escape or Tab puts it down, and a live region says
     the lift, each place and the end
     (decisions/beui_activating_drag_sources_keep_their_keys.md). It may give a site
     an optional node, which the inspector and `Into` nesting want.
   - **1d (done):** beui-demo pages: the list above, free positioning that snaps into slots
     (`closest(slots).when_far(free(area))`), and a switch dragged between its two states. An
     inspector tool, a dock tab like the others (guides/beui/programs.md, The inspector), that
     draws every candidate, its zone and the chosen one.
2. **Lists (done).** `slots_in` gives places among a list's rows, and one from another list at
   each gap; `Reorder` is built on the primitives and stays
   (decisions/beui_reorder_stays_over_drop_sites.md), and the database lists, kanban, schema,
   canvas layers and presentation slides use it. A row carried off its list collapses, so the
   `Reorder` draws its preview at its own root, where a screen reader finds it once. The files
   pane's tree moves to step 6.
3. **Dock tabs, fluid.** The drop highlight goes; the dock moves aside for a carried tab, the way a
   list opens a gap. In slices:
   - **3a (done): the dock on the board**, still with its highlight. Tab bars are `Reorder`s and
     `TabStrip` is gone; a pane's grip lifts through `Interactive`'s `drag`; `DockTiles` is a drop
     site of `custom` places, one over the home window's tiles and one per window the host opens,
     each answering the drop the dock's `resolve` gives at the point where the state admits it,
     so a locked place is no place and the refused marker is gone, with `Take` and
     `DropTarget`'s `takes`, which nothing else used. Each place answers only points over its
     own node that no layer but the dock's windows and dialogs covers, and a carry names its dock (`DockCarry`). `when_held(ALT, ..)` floats,
     and while held its places are the only ones; the board never chooses a place in another
     window than the pointer's; a carry let go outside every window is its source's
     (`Carry::outside`), which floats the tab there. A carried tab is listed once, as its bar's
     preview. Tool bars stayed on the old `DropTarget` until step 4.
   - **3b (done): the fluid preview.** The candidates, the preview, the carried tab, panels keeping their
     size and bars moving with their pane, below; the highlight and `resolve` are gone. The dock
     previews from `over`, so it needs no tentative gesture: its layout is view state. The spec
     language gained `Candidate::nearest_edge` (edge bands that split corners along the diagonal), `shaped(within)` (the item drawn the size of its place, under the pointer, kept inside `within`) and
     `open_where` (places reached only where nothing covers their site). A tab bar parting for a
     tab from another pane parts the dock too. Panes keep showing their real tabs until the drop.
   - **3c (done): panels stay built, and keyboard carries between bars**, below. A drag source
     hears the carry keys while the focus is on it or within it, so a dock tab's row (its tab
     button focused) moves with Ctrl+Shift and an arrow or Move, stepping into the dock's
     labelled places. An `Embed` moved between parents in one frame is never told it was
     unplaced, so an app dropped into another pane is placed once, at its new size.
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

   - **Panels stay built (done).** A pane whose panel moves to another place in the layout
     keeps its panel: each is already a `Kept` under the dock's one `KeepAlive`, which moves its
     subtree between parents (guides/beui/core.md); the dock's `panels` map is only an index of
     the built panels' nodes, which `KeepAlive` does not expose. What was missing was below the
     dock: the old pane's removal told its app's `Embed` it was unplaced before the new pane
     placed it.

   - **Tabs carried by the keyboard between bars (done).** A focusable source carried off its
     list by the keyboard keeps its focus: its row is hidden with opacity and drawn at no size,
     its slot closing with the gap, rather than `collapsed`, which blurs it on HTML
     (`display: none`) and leaves it unplaced natively. Pane grips stay unfocusable.

   - **`TabStrip` goes (done, 3a).** Step 2b moved the old `Reorder` unchanged into beui-dock-host as the
     private `TabStrip`, because the dock still splits panes from the old `DropTarget`, which
     only its drags reach. Tab bars become drop sites over `slots_in` like every other list,
     and `TabStrip` is deleted. This step comes right after step 5, so the copy does not
     linger.

   Expect a different feel, not a copy: replace the drop-highlight tests and paintings with ones
   that show the dock parting for a carried tab.
4. **Dock tool bars (done).** `bars.rs`'s band geometry is
   `closest(band slots, sticky).when_far(float.thrown())`, with the slots read from the bars'
   laid-out rects; bars beside a slot glide aside for the carried bar, and the tests that pinned
   the old thresholds pin the new reaches. The float is a `custom` place, not `free`: `free`
   clamps the item into its area, which would keep the bar's grip off the dock's edges, and
   where a bar floats depends on the pane under the point. Only the float is thrown, so a bar
   let go fast in a band docks there. Bar grips are focusable, so a keyboard carry steps
   between the band slots. It added `Carry::kept_in` (a bar drawn above its dock's own windows
   is read in the dock's window) and `PointerId::Relayed` (an app's grip moving its bar across
   the app protocol), and `when_far` now takes its far side wherever its near side chooses no
   place.
5. **Apps.**
   - **video timeline (done):** a `custom` site whose `place` is `drop_target_at`, so a clip
     lands at the frame under the pointer; `DropMarker` draws from the site's `over`. A keyboard
     carry cannot step into it yet (plans/backlog.md, Keyboard steps into a custom site).
   - **logic-grid's hotbar (done):** slots carry onto other slots and folders, each a place from
     `place_on`, which offers a node's whole area.
   - **infinite canvas:** item moves become `free`. A move holds its writes in a tentative
     gesture already (`CanvasState::open_hold`, guides/canvas.md, Gestures), so bound lines
     and every view on the device follow it while peers see it only on drop; on the board it
     applies the place it gets from `over` through that hold, and the board publishing the
     carry gives peers the move back while it is dragged. The board cancels a carry whose
     gesture the replica ended (`CanvasState::gesture_ended` lets go of the canvas's own
     gesture today). `free` also clamps the item into its area, which would keep a selection
     from being moved past the viewport's edge; a canvas site wants its area unbounded (the
     camera's world, scrolled by the edge band).
   - **game editor (done):** each `Gesture::Drag { from, to }` is a place at `to`'s spot,
     zoned to it, with the item drawn there the size it was lifted at; `drag` answers
     `None` off a piece, so the same press pans. The site draws the lifted piece from the
     board's frame (`drawn_by_site`); Space lifts the selected piece from the keyboard. Its presses are caught by an `Interactive` laid over
     the table rather than around it: a drag source's face takes its child's own size, so
     a child that fills whatever it is given collapses inside one (plans/backlog.md).
   - **between blocks (a step of its own, after the canvas):** an item dragged into another block of a type that takes it (canvas to
     canvas, list to list) moves on a plain drop and copies with a modifier, through the
     clipboard's typed formats, so a drop is a paste. Without write access to the source it copies.
     The source holds a tentative removal. On drop the target writes the insert first and the
     source then commits its removal, so a crash between the two leaves a copy, never nothing.
   - **settings' display arrangement (done):** every place a display touches another lined
     up with its side's start, middle or end (`touching::places`) is a candidate, and within
     `SNAP` of none it is `free` in the area; a free drop settles against the nearest edge
     as before.
   - **bottom sheet (done):** its release chooses among its detents and closing as
     `closest(detents).thrown_for(..)`, evaluated where it lets go rather than carried: the
     sheet's height and its content's scroll are one continuous travel under the finger,
     which stays on `on_drag` (step 8). Closing is a place below the lowest detent, so a
     release past the close distance takes it; `thrown_for` lets a flick carry on longer
     than a carried item's throw.
6. **Across apps.** plans/host_as_platform_adapter.md owns the cross-app session (Input, "Drag
   and drop"): the source starts it with its payload, and the host routes drag-over events to the
   surface under the pointer and the drop to the one under the release, by its forwarding router.
   This step owns what those events carry and what the surface does with them. Each app region is
   an `Into` candidate on the host's board. A drag-over carries the pointer and the payload; the
   app runs the same engine over its own candidates, draws its own preview, and answers what it
   accepts and the rect it chose, so the host glides the item there on release. The events carry
   only points, rects and opaque ids (guides/apps/host_services.md). `Editor::drag` and
   `accept_drag` become this handoff.
   - **The files pane's tree.** Its row drag is the host's block drag, which leaves the app
     for other editors, so it moves here rather than in step 2: `styled::Tree`'s `reorder`
     becomes a drop site whose folder rows are `Into` candidates zoned to their middle, and
     `Reorder`'s tree-only `hover`, `part`, `shut` and `resting` go. The dock has no app boundary, so it stays one level.
7. **Several carries at once.** Allow more than one carry, one per pointer, when the VR layer's two
   controllers (or touch) need it; steps 1 to 6 key carries by pointer but carry one at a time.
8. **Continuous drags.** Sliders, scrollbars, splitters and the colour area as `free` along an axis,
   quantised. Move each only if it is as good as today.
9. **Remove the rest.** Delete the old API. Rewrite components.md's Drag and drop, docking.md's
   Dragging, Dialogs and tool bars sections, and host_services.md, and replace `Draggable` and
   `DropTarget` in the list of behaviour-only pieces styled re-exports
   (decisions/beui_outside_crates_use_only_the_styled_layer.md) with what replaces them. Record
   decisions/beui_drags_are_reachable_places.md with what was rejected (building speculative
   states, blending view trees, a numeric optimiser, a drag mode that writes before release,
   implicit context arming glides).

## Open questions

- **Moving between sites** (decided in step 2: decisions/beui_a_move_between_drop_sites_is_the_apps.md).
  The receiving site's `commit` gets the payload and the app does the move: it removes the item
  from its home and inserts it at the chosen place in one edit, as it would for any other command.
  `Carry` has no `moved_away` callback. Within one tree, a row could move without being rebuilt
  by being a `Kept` under its key inside one `KeepAlive` around both lists, which moves a subtree
  between parents (guides/beui/core.md); decide when a caller rebuilding rows is too slow.
- **A carried item far from every site** (decided in step 2): a `Reorder` row that another
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
- **Rests at lift vs the reflow.** Choosing against the layout at lift keeps places still under the
  pointer, but rows shift once a gap opens. Pick one convention in step 1 and draw it in the
  inspector.
- **`Between`.** Nothing blends between discrete states today. Add it only if the switch or the
  dock opening in proportion to the pointer (step 3) needs it.
- **A site's content changing mid-drag** (a peer removes a pane): recompute that site's candidates.
