# Writing beui components

How to develop a new beui component at each layer (unstyled, styled, base), how text and themes
work, and how pointer input reaches a component. Read `guides/beui/overview.md` first, and check
`guides/beui/components.md` for a component that already does the job.

## Where things go

| Layer | File | Register | Re-export |
| --- | --- | --- | --- |
| Unstyled | `crates/beui-components-unstyled/src/<name>.rs` | `lib.rs` | the component, handles, state readers and supporting types, from `lib.rs` |
| Styled | `crates/beui-components-styled/src/<name>.rs` | `lib.rs` | its public API, from `lib.rs` |
| Base node | `crates/beui-core/src/base/<name>.rs` | `base.rs` | the component in `crates/beui-view/src/components/<name>.rs`, re-exported from `beui::reactive` |

Add each new component to beui-demo as well.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance. Compose it from base
components. For an interactive control this normally means:

1. Model controlled values and transient interaction values with signals.
2. Use one `Interactive` for pointer and touch interaction, with `focusable=true` for tab order,
   keyboard events, activation and focus state.
3. Publish the correct AccessKit role and state with `component_accessibility`.
4. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the reactive state needed
   to paint the control.
5. Return the root base node directly, so component state and framework slots attach to the node
   callers receive.

Do not put theme colours, fixed visual spacing, typography or decorative shapes in this layer. A
new skin should be able to use the unstyled control without undoing visual decisions.

State that belongs to the component's own handlers is captured directly. Use
`set_component_state` only when tests or host integration need to read the state from the
component's `NodeId`, and expose a focused helper such as `toggle_checked(&Document, NodeId)`.
Controlled state listens to its prop and reports user changes through its callback; see
`unstyled::Toggle` and `unstyled::TextInput` for the pattern, and `guides/beui/reactive.md` for
controlled state in general. Keyboard behaviour expected of each control family is in
`guides/beui/input.md`.

### Catcher props

An `Interactive` is a catcher. Besides its press, drag, click and hover callbacks:

- `cursor`: shown while the catcher is hovered or held. A catcher given none leaves the cursor to
  the catchers around it, so one that only listens (for a secondary press, a wheel) does not undo
  the I-beam of the text field it wraps.
- `on_scroll` takes the wheel and `on_scroll_drag` a touch drag; `scroll_axis` names the axis it
  takes them along, so a vertical wheel over a horizontal strip passes through to whatever is
  around it and a drag reaches the innermost catcher that scrolls that way. `unstyled::Scroll` is
  built out of those three.
- `capture_presses` / `capture_at`: stop a press at this catcher (see below).
- `claim_modifiers` / `claim_at`: claim presses made with modifiers held
  (`guides/beui/components.md`, "Buttons and presses").
- `hit_at`: turn down presses at some points, making the catcher transparent there.
- `touch_drags`, `touch_drag_axis`, `claims_touch`, `on_finger_tap`, `on_cancel`: see "Touch".

### How pointer input reaches a catcher

Each mouse button and each finger is a pointer of its own, and the document sees their downs, moves
and ups in the order they happened (`InputState::pointer_events`), so a press, a move and a release
that share a frame are three steps.

**Hit test.** A down is hit tested once (`interact::hit`). The layers of `Document::layers()` are
taken from the top by the rules of their policies, and in the first one the pointer lands on, the
hit is the topmost `Interactive` under it together with every `Interactive` around it: its path.
The same walk collects the layers the press is outside of, which it then asks to close. Any other
node is transparent, and so is an `Interactive` at a point its `hit_at` turns down, along with the
catchers around it up to one that captures or forwards. A passing layer whose content turns a point
down lets it through to what is beneath.

**Who gets the press.** The pointer keeps what its down reached until it is let go.

1. A modifier claim (`claim_modifiers`, `claim_at`) is looked for first, from the top of the tree
   down and past `hit_at`; a claimed press reaches its claimant alone.
2. Otherwise the press goes up the path from the topmost catcher and stops at the first that
   captures it (`capture_presses`, or `capture_at` at points its callback accepts).
3. Every catcher it reached that handles presses arms: it hears `on_press`, `on_drag` wherever the
   pointer goes (once in the frame of the press too), and `on_click` when released over the
   catcher, or for a finger, released without having been dragged.
4. The press then focuses the innermost focusable catcher it reached, or clears the focus, unless
   it was claimed or captured: those leave the focus to their catcher.

A secondary press reaches every catcher on the path that takes one, and a middle press the
innermost that pans, autoscrolls or forwards.

**Hover and wheel.** Hover is the path under the mouse, worked out again every frame, so a catcher
hears `on_hover_change` when it joins or leaves the path whether the pointer moved or the layout
did. The cursor is the innermost armed catcher's, else the innermost hovered one's. A wheel turn
goes to the innermost catcher on the path under where the pointer was that takes it along its axis,
unless a forwarding catcher comes first, and stays latched to it for `WHEEL_LATCH_TIMEOUT` while
the pointer holds still.

**Parts outside the rectangle.** Parts a catcher draws outside its own rectangle, such as the touch
selection handles of `unstyled::TextArea`, take their presses with `claim_at`. Paint order is the
tree's order, so paint such parts from a node that comes after whatever they must cover, or from a
`CanvasItem` with `clip=false`, which a canvas paints without cutting it to its own rectangle.
Those handles are carets with a handle, which `Text` paints after its text; a single-line area
paints them from an unclipped item after the text's.

### Touch

- **Finger drags.** A finger dragged across an `Interactive` is read as a scroll of whatever holds
  it that scrolls that way (where nothing does, it stays the catcher's drag) unless the catcher sets
  `touch_drags` (a canvas that draws or moves things under the finger) or `touch_drag_axis`, which
  keeps finger drags along one direction only and leaves the other to the scroll around it (a dock
  tab in a scrolling tab bar).
- **Settling a drag.** A finger drag is settled when the finger leaves the tap slop: the innermost
  catcher on its path that takes a drag that way (`touch_drags`, `touch_drag_axis`, or
  `on_scroll_drag` along its `scroll_axis`) wins it, every armed catcher that does not hold it
  hears `on_cancel`, and the winner's `on_scroll_drag` hears the rest. A claimed finger drag stays
  with its claimant.
- **Cancelling.** A second finger, a long press and a lost window focus cancel the press the same
  way. `on_cancel` is where a gesture in progress is dropped rather than committed.
- **Long press.** A finger held still for the long-press delay (`Context::set_long_press_delay`,
  which a test sets to zero rather than waiting) is a secondary press where it rests, so every
  context menu opens on tap-and-hold. The press the finger began is cancelled, and lifting it is not
  a click.
- **Touch reach.** A finger that lands on no control is taken to the nearest `Interactive` that
  takes presses within `TOUCH_REACH` of it, for the whole touch, so every control's touch zone is
  bigger than it looks. A direct hit always wins, so a neighbour never takes a tap aimed at the
  control beside it. A catcher that only watches presses over a whole region (a dock pane focusing
  itself) sets `claims_touch=false`, so landing on it is not a direct hit and the controls inside
  keep their reach.
- **Finger taps.** A quick tap with two or more fingers that did not move is a finger tap, which
  `on_finger_tap` hears the way `on_shortcut` hears keys, unless a layer that blocks everything is
  open. The editor frame's top bar undoes on two and redoes on three.

## Develop a styled component

An interactive styled component wraps the matching unstyled component, passes its `label` on for
the unstyled one to put in the accessibility tree, and renders the unstyled handle with base visual
primitives:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked label on_change>
            {move |handle: ToggleHandle| {
                view! {
                    <CheckboxFace handle />
                }
            }}
        </Toggle>
    }
}
```

The face derives colours and visibility with memos over `handle.checked`, `handle.hovered`,
`handle.active` and `handle.focused`, reads its text from `handle.label`, and composes `Frame` and
`Text`. Keyboard and pointer handling stay in the unstyled control. `styled/checkbox.rs` is a
short, complete example of the pair.

- Read colours from the nearest theme with `styled::use_theme()`, which returns a `ThemeStore`: one
  `ReadSignal` per token. Bind a token straight to a prop (`theme.accent`), or read tokens inside a
  memo that also reads interaction state, so the control repaints when either changes. Each token is
  its own signal, so a component wakes only for the colours it uses.
- Radii and spacing are constants in `styled::theme`.
- Add a field to `Theme`, with a value in every built-in theme, when a colour is part of the theme
  rather than unique to one component.
- Styled controls must visibly show keyboard focus, and keep labels and accessible roles stable when
  visual state changes.
- Icons are glyphs from `beui::icons` in a `styled::Icon` inside a `styled::Text`.

## Components that return a value

A child needs no node. A component that returns a value rather than a node still runs in a scope of
its own, which lives as long as the scope that built it, exactly as a node's does: a value built
inside a `show` branch or a `for_each` row goes with that branch or row, whatever happens to the
value. So an item made of data (a label, a key, a callback) can still be a component, with its own
scope, context, memos and cleanups, and still be written as a tag.

- Such a component has nothing for `component_state`, `component_accessibility` or
  `component_placed` to watch, so calling any of them in its body panics when it is built.
- `@test_id` and `@node_ref` on its tag do not compile: they only take a component whose output
  implements `BuildsNode`.
- Declare the type with `value_child_type!`, which makes it a `ChildValue` and says how a run of it
  is kept, so a `show` or a `for_each` can build one. A generic type names its parameters first:
  `value_child_type!(<K> DockNode<K>)`.

`unstyled::MenuItem` is one: a label, an icon, a detail, disabled and danger flags, a separator
above it, an `on_click` and its own submenu items, so a menu is written as tags and each row
follows the signals its tag was given. `unstyled::ChoiceOption` is the same for the options of a
tab bar, a listbox, a radio group and a select.

## Text

### Base `Text`

A base `Text` takes concrete values and knows nothing about the theme:

- `font`: a `FontId` (14 points proportional unless given). A `FontId` carries `bold` and `italic`
  beside its size and family; FreeType synthesises both rather than loading separate faces.
- `color`, `underline` (painted from the galley's baseline, so switching it on never moves
  anything).
- Paragraph props: alignment, `wrap`, `ellipsis`, `clip`, `line_height`, `line_padding`, `marks`,
  `carets`.
- `string`: a leading span in the Text's own style.

Its content is its children:

| Child | Is |
| --- | --- |
| string literals, `Prop<String>`s | text in the Text's own style |
| `Span` | a value child with no node, overriding its Text's `font` whole or any of `size`, `family`, `bold`, `italic`, and its `color`, `underline` or `strikethrough`; `break_after` marks a wrap opportunity. An unset span prop takes its Text's. |
| `Icon` | one glyph from the icon font (below) |
| `TextItem` | a child laid inline as if it were one character (U+FFFC in the text); given `at`, it is placed at that byte index as a caret would be instead of in the flow, which is how something floats beside a position in the text |
| `Gap` | room of a given width that holds no bytes, so the indices either side of it are the same |

Computed runs are a `ForEach` of `Span`s over the caller's own tokens.

An `Icon` paints one glyph and, like a `Gap`, holds no bytes, so the caret steps over it, a
selection never covers it and a copy never carries it: `<Text><Icon glyph=ICON_SAVE />"Save"</Text>`
reads and copies as "Save". `Document::text_icons` lists a text's icons.

- Its size is its Text's font size times `scale`, unless given a `size`.
- It is centred in its line, not set on the baseline, and makes its line no taller than text of the
  size it scales from would.
- It keeps `spacing_em` (a fraction of its own size) from whatever sits beside it on its line, so
  the gap between icon and label is layout, not a space a selection could take. An icon alone keeps
  none; an icon whose glyph is empty takes no room.
- Underline and strikethrough never cross one.

How the Text lays out:

- It lays the runs out and wraps them itself, breaking after whitespace, `-`, `/` or `\`, or after
  a `break_after` span, and between letters when no such break fits, with `line_padding` above and
  below each line. It aligns, truncates with an ellipsis and spaces lines by `line_height` whatever
  its content.
- It caches each span's shaping, so a span whose props change is the only one shaped again, and
  line breaking resumes from that span's line. A colour or decoration change only repaints.
- It paints `marks` behind the text (a selection, a code background) and `carets` in front of it,
  blinking ones included, as byte ranges over the joined content.
- `Document::text_geometry` answers where an index is and which index is under a point, from the
  node's last layout, without a document installed.

`unstyled::TextArea` is built from exactly this: a row per document line in a `VirtualList`, each a
`ForEach` of the row's spans, a gutter of `Text`s beside it, checkboxes, embedded widgets and the
caret's anchor as `TextItem`s, and table spacers as `Gap`s.

### Measuring text outside a node

`layout_text(text, font, layout)` lays text out with the shown document's fonts and answers `None`
until the document has been shown once. It is how a component measures text outside `measure` and
`paint` (where a caret sits, how wide a column is), and the galleys a `Drawing` paints come from
it. Its galleys are cached, so asking for the same word twice costs a hash lookup.

### Styled text

Styled text is written in tokens, never pixels or colours. `styled::Text`, `styled::Span` and
`styled::Icon` take the same children as the base ones and resolve their tokens into a base Text's
`font` and `color`:

- `size`: a `TextSize` from `Xs` to `Xl5`. `TextSize::px` is the point size, for the rare layout
  that must measure against it.
- `weight` (`Weight::Normal`, `Weight::Bold`), `family` (`Family::Sans`, `Family::Mono`), `italic`.
- `color`: a `TextColor` role (`Default`, `Muted`, `Faint`, `Accent`, `OnAccent`, `Danger`,
  `Warning`, `Success`) resolved through the nearest theme, so the text follows a theme change.
  `TextColor::Custom(color)` is for colours that are data rather than design (a user's label
  colour, a map marker, a terminal's ANSI palette). A memo that picks a colour by state returns a
  `TextColor`.
- `style`: a `TextStyle` preset: `BODY`, `CAPTION` (small and muted), `HEADING`, `TITLE`,
  `DISPLAY`, `CODE` (small and monospace), `PARAGRAPH` (muted; give it `wrap=true` as well). A prop
  given beside a preset wins over it.

A styled `Icon` is `ICON_SCALE` times its text's font size unless given a `size` token, and keeps
`ICON_SPACING_EM` from its neighbours, so an icon beside a label is one `Text`, not a row of two:

```rust
view! {
    <Text style=TextStyle::CAPTION>
        <Icon glyph=ICON_SCHEDULE />
        "Edited five minutes ago"
    </Text>
}
```

A lone icon is a `Text` holding only an `Icon`; give it `align=TextAlign::Center` when placed in a
box bigger than itself.

`TextStyleProvider` gives every styled text created within it a style its own props override (a
disabled section muting everything in it). It is context, so it reaches what is *created* inside
it, which is not always what is laid out inside it: text a render callback builds for an overlay
takes the provider around where the callback was written. A `Span` or `Icon` inside a `Text` takes
what it does not set from that `Text` instead.

Raw pixel sizes and `Color32`s belong to base `Text`, for text that is drawn rather than designed:
labels on a zoomable canvas, a terminal, a game board.

## Themes

`styled::Theme` holds the colour tokens. The built-in themes are `Theme::DARK`, `Theme::OLED` (dark
on pure black, so OLED screens leave most pixels off), `Theme::LIGHT` and `Theme::EINK`;
`Theme::BUILT_IN` lists them with their names. A theme's `kind` says which of these it is or
derives from, for code that must tell them apart without comparing colours.

Every `Document` owns a theme that styled components use when no provider covers them. Change it
with `set_theme` and read it with `theme`, both from the `styled::DocumentTheme` trait (for example
to pick an app's clear colour). The inspector's Sim tab switches it at runtime.

`ThemeProvider` overrides the theme for the subtree between its tags and follows the `Prop<Theme>`
it is given:

```rust
view! {
    <ThemeProvider theme={theme_signal}>
        <Settings />
    </ThemeProvider>
}
```

## Develop a base component

Base nodes are the only layer that should normally mutate a `Document` directly. Add one only when
the overview's test for a new base component is met. A base implementation has three parts:

- A node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction, child traversal and
  inspector metadata. Its `remove_child` lets go of a child whose scope went while the node still
  held it, so a node holding one child keeps it in an `Option`; a `ChildHost` hands it to
  `remove_hosted`.
- `Document::create_*` and `Document::set_*` methods beside the node, plus a public
  `#[component]` wrapper in `beui-view` that creates the node with `with_document` and binds
  reactive props to setters with `create_effect`.

The `base` module is private. Re-export base components meant for composition from
`beui::reactive`, as `Frame`, `Text` and `Scroll` are, and keep implementation-only primitives
crate-private when they exist only to support an unstyled control.

### Node handles

`Document::create_*` returns a `NodeOf<XNode>`, a node id that carries the kind of node it names,
and the setters take that handle, so a setter cannot be handed a node of another kind. `.id()` is
the plain `NodeId`. Code that only holds a `NodeId` (from a `NodeRef` or a tree walk) asks
`document.arena.kind_of::<XNode>(id)`, which answers `None` for another kind, and a query meant for
such code takes a `NodeId` and answers `None` or `false` itself, as `is_overlay_open` does.

### A new kind of parent

The child types of the node parents (`ListChild`, `GridCell`, `Layer`, `CanvasItem`, `TextItem`)
are all `Placed<P>` for the parent's node type `P`: a node and the `P::Data` the parent keeps
beside it (a `Sizing`, a span, or nothing). Each is a distinct type, so a child built for one parent
does not fit another. A new parent implements `ParentKind` for its node type, and `BareChild` too
when a bare `NodeId` should go in it unchanged.

### Measure and layout

- `measure` takes `&self` and must not mutate. `layout` takes `&mut self`, may update the node's
  own retained state, and may build its own subtree through `Document::build_within`
  (`guides/beui/views.md`, "Building during layout"), which is how a `VirtualList` builds rows.
- Both receive `&mut Document` and are reached through `beui_core::layout::measure` and
  `beui_core::layout::layout`, which take the element out of the arena for the duration. Reach
  children through those two functions, never by calling another element's methods directly.
- `unplaced` is the other half of `layout`. The layout calls it on every node it placed last pass
  and did not place this one (a child its parent now skips, and everything under it), so a node
  keeping retained state for what is on screen can let it go. It runs in the same pass, after the
  dropped rects are gone, and may remove the node's own children (how a `VirtualList` scrolled out
  of view releases its rows). A node laid out again gets `layout` as usual and rebuilds what it
  released.
- Measurements are memoised per available size and dropped whenever the node or something under it
  changes its layout, so measuring a child repeatedly within a pass is cheap, but a `measure` that
  is not a pure function of the node and its constraint returns a stale answer.
- A memoised size also answers a smaller offer it still fits: a node measured at bounded width `a`
  to `s` answers any bounded width between `s` and `a` with `s`, and a node measured unbounded
  answers an offer of exactly `s` with `s` (likewise for height). That keeps a list from measuring a
  child again at the length it just measured, so `measure` has to agree: offering a node more room
  than its size must not change its size unless that room is unbounded.
- `baseline` answers where the node's first line of text sits below its top for an offered size,
  for a row aligned to `Align::Baseline`. It defaults to none; `Text` answers from its first line;
  a node wrapping one child forwards the child's answer through `beui_core::layout::baseline`
  (adding whatever it shifts the child down by). Answers are memoised beside the measurements.
- Layout stays linear in the number of nodes: a node measures each child a bounded number of times
  per pass, and anything computed across children is linear too. `base::share` hands a length out
  by weight between minimums and maximums in linear time; a list's shrinking and percent children
  and a grid's fraction columns use it. The test
  `nested_lists_measure_each_node_a_bounded_number_of_times` holds a deep tree of lists to that.
- A node whose size cannot depend on its children (a `Frame` with fixed width and height, a
  `Canvas`, a canvas item) returns true from `relayout_boundary`. A change under it stops there: its
  ancestors keep their placement, and it is laid out again on its own from the rectangle and
  painter it was last placed with. Return true only when `measure` never reads the children.

### Setters

A setter reaches its node through one of these arena accessors, chosen by what reads the field.
Only call them when the value actually changes.

| Accessor | For fields read by | Effect |
| --- | --- | --- |
| `get_mut_as` | `measure` or `layout` | the node and its ancestors are measured and laid out again |
| `paint_mut_as` | only `paint` (a colour, a tint, a drawing) | that node alone paints again; nothing is laid out |
| `touch_mut_as` | neither (handlers, a cursor, a tab stop) | only the accessibility tree hears of it |
| `state_mut` | pointer-dispatcher state on an `Interactive` (hovered, armed, panning) | no one hears of it; what it changes reaches the screen through the callbacks it calls |

A field `paint` reads through something `layout` computed, like the origin a `Text` places from its
alignment, counts as layout.

### Coordinate spaces

Every node is laid out and painted in a coordinate space of its own, starting at its top left:
`layout` and `paint` are handed a rectangle at the origin, of the node's size, and the rectangles
`layout` gives its children are in those coordinates too. A node that only moves (a row pushed down
by the one above growing, a panel beside a splitter) keeps its layout and recorded shapes; its
parent records where it went.

A scroll's `OffsetNode` and a `Canvas` add a content space inside their own
(`Document::enter_space`, and `Painter::shifted` when painting) that carries the scroll offset or
the pan, so scrolling or panning changes only that translation and the rows are neither laid out
nor painted again.

`Document::node_rect`, `PlacementBuilder`'s rect, the rect `interact` is handed and a `CanvasView`'s
origin are in document coordinates. A node that needs to know where it sits in them asks the
painter: `painter.origin()` is where its space starts and `painter.clip_rect()` is what of it is
visible, in its own space. Asking either marks the node as depending on its place, so it is laid out
or painted again when that changes (an `Embed` reports its rectangle this way; a `VirtualList`
realises the rows its clip shows).

### Painting and damage

Painting is retained per node. A node's `paint` runs again when the node changed, when its size or
the painter it is handed changed, when a child it paints moved, was culled or uncovered, or when a
repaint it asked for falls due. Otherwise its recorded shapes stand, and its children are visited
only when something under them has to paint.

- Being laid out again is not by itself a reason to repaint. A node whose paint reads something its
  own `layout` works out says so with `Element::paints_from_layout` (`Offset`, `Canvas`,
  `VirtualList`), and a `Text` asks for its own repaint from `layout` only when the lines it placed
  differ from before.
- So `paint` must be a function of the node, its rectangle, the painter and the rectangles its own
  layout gave its children; anything else it reads goes unnoticed when it changes.
- A scroll's new offset relays out its `Offset` alone, from the place it was last given.

What a node records is a tree (`display.rs`): its own shapes, its children by id and translation,
and group nodes around them (`Group::Clip`, `Group::Scroll`, `Group::Mask` for a fade,
`Group::Transform`, `Group::Filter`). A painter opens groups (`with_clip_rect`, `shifted`, `faded`
with the space of the fade that opened it, `in_document`) rather than writing them onto each shape
or child: a recorded shape's own `clip` is always everything, and `FrameOutput::shapes` folds the
groups into it. `PainterState::within` is what each group does to the painter inside it, so a node
visited without painting again hands its children the painters its tree says they had.

When a node paints again its new tree is set against its old one by identity, never by comparing
paint output:

- its own shapes damage where they were and where they are, except a `Drawing` that is the same
  drawing in the same place, which damages only what it says it redrew;
- a child, matched by id, damages its old and new place when added, removed, moved or restacked,
  and nothing when it stayed;
- a mask whose fade changed damages its bands;
- a scroll group, matched by the space it scrolls, whose offset changed is a copy of what it showed
  rather than damage.

A repaint that paints the same thing still damages its shapes, so a node that repaints without
changing is fixed where it asks to repaint. Damage is held as at most four rectangles
(`damage::Region`), each drawn as its own pass; two become one whenever their bounds are at most
twice the area they cover.

A frame hands the renderer these trees, with the frame's own groups around them
(`Context::clipped`, `Context::scaled` and `Context::apply_filter` wrap what they cover), rather
than a flat list; `FrameOutput::shapes` flattens them on demand, for tests and software
rasterising. The `Renderer` encodes a node's shapes once and keeps their instances in a GPU buffer,
walks only the trees that reach the damage, and draws a scroll group through a translation and clip
of its own, so a scroll or pan encodes nothing again.

A scroll whose offset changed reports its copy as `FrameOutput::moved`. A host that keeps its last
frame, as `beui::run` does, copies that region by the scroll with `Renderer::shift` and repaints
only what the copy cannot supply: exposed rows, whatever changed, and whatever does not move with
the rows (a fade's bands, a scrollbar drawn over them). Only one region is copied per frame: when
several scrolls moved at once the largest is copied and the rest repainted. `FrameOutput::repaint`
covers the moved region for hosts that do not copy.

### Checking paint and damage

beui's own tests, and every test that drives a plugin through `block-ui-test` (which turns on
`beui::verify_paint`), paint every frame again from scratch and fail when the retained painting
differs from it or changed outside the damage. The second check (`paint/verify.rs`) sets the frame
before against this one node by node, matched by id: a node's own shapes, the fades over it and its
place among its siblings are what it shows, and every pixel a difference touches, rounded out to
whole pixels, must be damaged or, inside a region the frame copied, must be what the copy brought
there. Painting from scratch runs every paint again, so a test that counts paints skips those made
while `Context::is_verifying_paint` is set.

With `BEUI_OVER_REPAINT=1` in the environment (or `beui::detect_over_repaint(true)` on the painting
thread) they also report every frame whose damage is more than four times the area of the shapes
that changed, to stderr and to `beui::take_over_repaints`.

### Pointer traversal and metadata

Pointer input visits a node only when the pointer lies within the rects of it and everything
`children` returns under it, or when it leads to a node holding pointer state. A node that keeps
state between events (hovered, pressed, dragging) returns true from `engaged` until it lets go, so
it still hears the pointer leave, release or move away. Keyboard input goes to the focused node and
its ancestors rather than through this walk.

Return children from both interaction traversal and `children`, give the node a stable `kind` for
the inspector, and add a concise `detail` when it makes the tree easier to understand.

## Testing components

Running tests, `BeuiTest`, test ids and paintings are covered in `guides/testing.md`. Specific to
beui's own crates:

- Library behaviour is tested headlessly: add a test under the relevant `tests` directory and
  declare it in that directory's `tests.rs`.
- The document tests use their `Harness` to build a `Document`, send `Event` values through a
  `Context`, and inspect component state, layout, accessibility or painting output.
- A test that needs several screens sets them with `Context::set_screens`; one that exercises long
  press sets `Context::set_long_press_delay` to zero.
- Paint verification runs in these tests automatically (see "Checking paint and damage").
