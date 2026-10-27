# An HTML core for beui on the web

beui on the web gets a second core. This file is the inventory of the base
layer both cores implement, the proposal for what that layer is, and the design
that built it. Implemented (guides/beui.md, Cores), except what "Left" at the
end lists.

## The decision

- **Two cores, chosen at compile time.** Native platforms keep beui's own core:
  `beui-core`'s retained nodes and the `beui-view` primitives over them. The web
  gets an HTML core: the same base components implemented with real DOM
  elements, through a live `web-sys` backend of `beui-html` (which today only
  renders HTML strings for SSR, on `beui-tree`). The browser does layout (CSS),
  focus (real DOM focus, `tabindex`, `:focus-visible`), input and selection.
  That buys SSR with partial hydration, browser-owned focus, autofill and
  password managers, find-in-page, and a smaller download.
- **Unstyled and styled are shared.** They reach base only through
  `beui-view`, never `beui-core`. Where a real HTML element should replace
  beui's own (a native scroller later), unstyled and styled switch on the
  core at that one component, through one compile-time constant `beui-view`
  exports. Such a switch is a deliberate choice per component, not a way
  around a missing base primitive.
- **The base layer must be small.** Every base primitive costs two
  implementations. It has drifted (§4).
- **The contract.** The base API's shape admits what every core needs (late
  answers, browser-owned focus); each core's behaviour is the best its
  platform allows; no feature exists on only one core. Native must not
  regress: virtual scrolling stays frame-perfect, measurement and scroll
  anchoring stay synchronous. Only callers change, so they stop assuming an
  answer arrives in the same frame. On the web, `ResizeObserver` callbacks run
  after layout and before paint in the same frame, so DOM virtualization can
  be jump-free too; the real web gap is compositor-thread scrolling showing
  blank space on fast flings, which overscan mitigates. CSS anchor positioning
  covers anchored popovers. A test-only option that delays measurement in
  headless keeps the late path exercised on native.
- **`Drawing` is inline SVG on the HTML core.** The first version drew pink
  TODO rectangles; now the `Draw` callback paints into an SVG `Painter`, so
  a server-rendered page shows drawings before wasm loads, which a
  `<canvas>` could not. `Picture` and the scrollbar thumb stay `Drawing`s on
  both cores (one way to paint an image or a thumb): an SVG `<image>` and
  `<rect>` on HTML. The colour wheel's gpu face is computed on the CPU into
  an image there.
- **Text editing stays in unstyled.** `TextArea` and `TextInput`, with
  `text_editor_core`, rows, `VirtualList`, completion and handles, are shared
  by both cores. They rely on base `Text`'s declarative props, which the HTML
  core implements: `marks` as the CSS Custom Highlight API (`::highlight()`),
  `glyphs` as `<span>`s replacing their range, `carets` as positioned
  elements placed from a DOM `Range`'s rect, `hang` as a negative
  `text-indent` with `padding-left`. Two base contracts are new: text
  geometry (point to offset, offset to caret rect, line boxes), which native
  answers from `RichLayout` and the HTML core from `caretPositionFromPoint`
  and `Range.getClientRects()`, synchronously in both; and text input and
  IME on `Interactive`, which the HTML core feeds from the hidden
  `<textarea>` `beui-platform-web` uses today, and later from EditContext.
  Offsets must agree across cores (UTF-8 in Rust, UTF-16 in DOM ranges, and
  how a `glyphs` replacement counts). Line breaking and shaping come from the
  browser on the HTML core, which `beui-font-browser` (written for the DOM
  renderer this plan replaced) already measures; the editor only asks for geometry and
  never computes it. Editors get real DOM text, so find-in-page, translation
  and screen readers see their content.
- **Read-only selection is the browser's.** Selection is a core feature
  (`beui-view`'s `selection.rs`), so on the HTML core display text is
  selected and copied by the browser. Inside editors `TextArea` owns
  selection on both cores.
- **No `<input type=password>` in the first version.** `TextInput`'s
  `password` mode stays beui's own. A real password input, for
  password-manager autofill, is a follow-up switch in unstyled, like
  `Scroll`'s.
- **The first version keeps owning scrolling.** `Scroll` stays in unstyled,
  built on `Offset` and `Interactive`. The HTML core implements `Offset` as an
  `overflow: hidden` element moved through `scrollTop` and `scrollLeft`, not
  a transform, so find-in-page, `scrollIntoView`, `#anchor` links and
  `position: sticky` still work. A follow-up makes unstyled and styled switch
  to a real `overflow: auto` scroller on the HTML core; that changes who
  drives the scrolling, not the element tree, and is not a core change.
- **Out of scope.** Plugin editors stay on canvas/wgpu on the web for now
  (they run in workers). Later, web plugins become iframes, which could run
  beui on the HTML core.

## How the counts below were taken

A script walked `crates/` and counted files containing a `view!` tag `<Name`
preceded by whitespace, a brace, a bracket, `>` or `,` (so `Vec<Turn>` and
`Option<Window>` don't count). Test files (`tests/`, `tests.rs`) are skipped,
and so is `beui-components-styled/src/lib.rs`, which only re-exports. Styled
shadows `Frame`, `List`, `Grid`, `Text`, `Span`, `Icon`, `Gap` and `Root` with
its own; in styled and outside beui those count only where the file names
`primitives::X` or glob-imports primitives without importing the styled one,
so they are undercounts if anything. Columns: **U** unstyled, **S** styled,
**B** other beui crates (`beui`, `beui-demo`, `beui-dock`, `beui-inspector`,
`beui-keyboard`, `beui-layer-*`, `beui-platform-*`, `beui-renderer-*`; not
`beui-core`, `beui-view`, `beui-html`, `beui-tree`), **O** outside beui.
beui-demo's `base_pages.rs` is one of the B files for nearly every row.

## 1. Base primitives

HTML column: **CSS** (CSS-native, which CSS), **late** (needs the async-shaped
contract; what native keeps), **hatch** (core-implemented), **unclear**.

| Primitive | What it does | U | S | B | O | HTML core | Merge / remove |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Frame` | one child: size (`Extent`), aspect ratio, padding, align, fill, radius, outline, overlay `pointer` beak (moving to the overlay) | 12 | 3 | 6 | 0* | CSS: box, `aspect-ratio`, `padding`, `place-self`, `background`, `border-radius`, `outline`+`outline-offset`. The beak moves to the overlay (§5) | Absorb `Shift`, `Turn.angle`, `Fade` as props |
| `List` | children in a line: direction, align (incl. baseline), justify, wrap, spacing; children's `@sizing` | 28 | 1 | 5 | 0* | CSS flexbox: `flex-direction`, `align-items: baseline`, `justify-content`, `flex-wrap`, `gap`; `Percent` is `flex-grow` from basis 0, `shrink` is `flex-shrink`, `min`/`max` are `min-`/`max-width`; beui's min-width rule is CSS's `min-width: auto` | — |
| `Spacer` | an empty `Frame` | 1 | 4 | 5 | 31 | CSS: empty flex item | Keep: fills the rest of a row (`Percent`, ~70 uses) or stands in for nothing (bare, 51). Its ~6 fixed-pixel uses outside beui become a styled `Spacer` taking a `Space` token, as styled `Gap` does |
| `Layers` / `Layer` | children over one box, sized to the largest | 3 | 3 | 4 | 24 | CSS grid, every child in `grid-area: 1/1` | — |
| `FirstFit` / `Fitting` | the first child whose natural width fits, else the last | 0 | 1 | 1 | 0 | **late**: measure natural widths (`ResizeObserver` on `max-content` probes); native keeps same-frame choice | One user (`styled/choice.rs`). Move to unstyled over the measurement API |
| `Grid` / `GridCell` | shared columns of `Track`s (fixed, intrinsic, fraction), spans | 1 | 1 | 1 | 0 | CSS grid: `grid-template-columns` px / `max-content` / `fr`, `grid-column: span n` | — |
| `Text` | a paragraph: font, colour, align, `Overflow` (wrap, max lines, ellipsis, visible), line height and padding, `hang`, `marks`, `glyphs`, `carets` | 1 | 2 | 2 | 0* | CSS for display text: `white-space`, `text-overflow: ellipsis`, `line-clamp`, `overflow-wrap: anywhere`, `line-height`, `tab-size`. `marks` as `::highlight()`, `glyphs` as replacing `<span>`s, `carets` as positioned elements; text geometry from `Range` | Pin down `marks`, `glyphs`, `carets` for both cores |
| `Span` | styled run inside `Text`, with `background` and `break_after` | 2 | 2 | 2 | 0* | CSS: `<span>`, `box-decoration-break: clone` for the background; `break_after` is `<wbr>` | — |
| `Icon` | one icon-font glyph, holds no bytes | 1 | 1 | 1 | 0* | CSS: `<span aria-hidden>` in the icon font, `user-select: none` | — |
| `Gap` | inline room holding no bytes | 1 | 1 | 1 | 0* | CSS: `inline-block` with a width, `user-select: none` | — |
| `Break` | line break that copies with no newline | 0 | 0 | 1 | 1 | **unclear**: `<br>` copies a newline; a zero-size block span may not | Only the terminal-style soft wrap needs it |
| `TextItem` | a node laid inline as one character, or placed at a text index (`at`) | 1 | 0 | 0 | 0 | Inline: CSS `inline-block`. `at`: **late** (index to rect) | `at` has one user (`text_area/lines.rs`): editing only |
| `Offset` | shows its one child from an offset; no input; scroll anchoring, `follow_end`, reveal | 2 | 0 | 0 | 0 | CSS: an `overflow: hidden` element moved through `scrollTop`/`scrollLeft` | Only `unstyled::Scroll` and `Sheet` use it. Stays shared; `Scroll` stays in unstyled |
| `VirtualList` | builds only rows the viewport reaches; remembers measured rows; anchors the scroll | 1 | 0 | 1 | 6 | **late**: row sizes from `ResizeObserver` (same frame, before paint), overscan for flings; native keeps synchronous measurement and anchoring | — |
| `Canvas` / `CanvasItem` | a pan/zoom view of absolutely placed items, raster settling | 2 | 0 | 2 | 22 | CSS: absolutely positioned items in a container with `transform: translate() scale()`; raster settling falls to the browser. `on_reveal` is **late** | — |
| `Drawing` | a `Draw` callback handed the `Painter` and its rect | 3 | 2 | 1 | 12 | **hatch**: a pink TODO rectangle first; `<canvas>` or SVG later | Unstyled uses it for a scrollbar thumb and `Picture`'s image; neither should be a canvas on HTML (§4) |
| `Overlay` | a layer anchored to a node, rect or point: placement, scrim, policy, level, focus trap, dismiss, keep-alive | 11 | 0 | 4 | 2 | CSS + top layer: `popover`/`<dialog>`, CSS anchor positioning with `position-try-fallbacks`, `::backdrop` for the scrim, `showModal()`/`inert` for `BlocksAll` and the trap | `Window` is an `Overlay` mode |
| `Window` / `WindowHandle` | an `Overlay` that becomes an OS window where the platform opens windows | 0 | 0 | 4 | 4 | CSS (as an `Overlay`); the web opens no windows | Keep as a `beui-view` composition of `Overlay`, not a primitive |
| `Interactive` | focus, keys, text, IME, paste, pointer, gestures, hit tests: 61 props | 23 | 0 | 4 | 22 | DOM events for focus (`tabindex`, `focus()`, `focusin/out`), keys, pointer (`setPointerCapture`), paste. IME through the hidden `<textarea>`, later EditContext. Custom hit tests (`hit_at`, `capture_at`, `gestures_at`, `drawing_at`, `claim_at`): **unclear** | Split; move gesture recognisers up (§4) |
| `Embed` | reserves a rect for something composited outside the document, punches a hole, reports placement | 0 | 0 | 0 | 3 | **late** (placement via `ResizeObserver`); the content would be an iframe | Product-only (block-app, block-editor-beui); plugins are out of scope |
| `Portal` | shows a subtree it doesn't own, moving it without a rebuild | 2 | 0 | 2 | 3 | DOM: move the element (`moveBefore` keeps focus and iframe state) | — |
| `BackHandler` | the innermost enabled handler gets Back and the back gesture | 2 | 1 | 0 | 3 | No layout. `popstate`/history and Escape; the back gesture has no web equivalent | A pass-through node for routing only: make it a hook like `on_key` |
| `Shift` | moves its child by a vector without moving its box | 2 | 0 | 1 | 0 | CSS `transform: translate()` | `Frame` prop |
| `Turn` | quarter turns (box swaps width and height) and an angle (box unchanged) | 0 | 0 | 1 | 2 | Angle: CSS `rotate`. Quarter turns: **late** (the swapped box needs the child's size) | No unstyled or styled user. Angle becomes a `Frame` prop (with `Embed.rotation`); quarter turns go to their two users |
| `Fade` | opacity, and fades toward the edges | 2 | 1 | 2 | 1 | CSS `opacity` and `mask-image: linear-gradient` | `Frame` props |
| `LayoutBuilder` | builds content from the size it is laid out at, in the same layout | 4 | 0 | 2 | 5 | **late** (`ResizeObserver`); native keeps same-frame | One of five ways to ask layout (§2.2) |
| `PlacementBuilder` | hands content the rect it is placed at | 2 | 0 | 1 | 9 | **late** | Same answer as `node_rect`; merge |
| `Root` | the window's clear colour, title, system bars | 0 | 1 | 1 | 0* | DOM: body background, `document.title`, `theme-color` meta | — |

\* Outside beui, `Frame`, `List`, `Text`, `Span`, `Icon`, `Gap` and `Root` are
the styled ones; styled re-exports every other primitive unchanged, which is
how outside code reaches `Interactive`, `Canvas`, `Layers` and `Drawing`.

**Not primitives**, though they live in `beui-core/src/base`:

- `focus.rs` (`Focus`, `ImeCursor`, `focus_within`): the focusable half of
  `InteractiveNode` and the document's focus logic.
- `focus_scope.rs` (`FocusScope`): pushed by overlay layers, and by block-app's
  compositor (`compositor/region.rs`) through `Document::push_focus_scope`, its
  only direct user.
- `share.rs`: the weight-sharing algorithm `List` and `Grid` use.
- `child_list.rs`: child storage for multi-child nodes.

These are core internals. Move them out of `base/` so the directory lists only
what both cores implement.

## 2. Direct `beui_core` use in unstyled and styled

65 unstyled and 63 styled source files import `beui_core`. 15 unstyled and 55
styled files import only plain values.

### 2.1 Plain value types (mechanical move)

`NodeId` (54 unstyled / 60 styled), `Color32` and colour spaces, geometry,
`icons`, `Direction`, `Align`, `Justify`, `ItemSize`, `Sizing`, `Sides`,
`Extent`, `Track`, `ScrollPosition`, `TextAlign`, `Overflow`, `FontId`,
`FontFamily`, `Image`/`ImageFit`/`Thumbhash`, `Corners`, `SystemBars`,
`ThemeKind`, `PickedFile`, `PasteKind`/`Pasted`, and the input values (`Key`,
`KeyPress`, `Modifiers`, `CursorIcon`, `PointerPress`, gestures; 33 unstyled
files, 1 styled), the key routing values (`KeyAt`, `KeyPhase`, `KeyEvent`),
and the layer values (`OverlayAnchor`, `Placement`, `LayerPolicy`,
`LayerLevel`, `Dismiss`; 14 unstyled, 3 styled). Many of these already come
from `ui-types` or are re-exported by `beui_view::reactive`.

### 2.2 Layout queries

Through `beui-view`, not `beui_core` directly, but the real leak: they assume
an answer from this frame's layout. 9 unstyled files, 0 styled.

- `placements().rect(..)`: `sheet.rs`, `text_area.rs`, `selectable.rs`,
  `context_menu.rs`, `scroll.rs`.
- `node_rect`, `node_placed`: `tree.rs`, `text_area/lines.rs`.
- `text_bounds`: `text_area/lines.rs`.
- `viewport_rect`, `use_viewport_size`: `back_slide.rs`, `dialog.rs`.
- Text geometry (`text_geometry`, `caret_rect`, `text_index_at`,
  `layout_text`, `set_text_selection*`): `text_area.rs`, `text_area/state.rs`,
  `text_area/lines.rs`, `text_area/rows.rs`, `selectable.rs`.

With `LayoutBuilder` and `PlacementBuilder` that is five ways to ask the same
kind of question. Outside the editing components only `sheet`, `context_menu`,
`scroll`, `tree`, `back_slide` and `dialog` need one.

### 2.3 `Painter` outside `Drawing`

None. Every `Painter` in unstyled (`scrollbar.rs`, `picture.rs`,
`text_area/lines.rs`) and styled (`color_picker.rs`, `color_wheel.rs`,
`color_wheel/gpu.rs`) is inside a `Draw` callback. Styled's colour wheel also
holds a `beui_core::drawing::Drawing` (a GPU drawing), which is a hatch.

### 2.4 `rich::` internals

7 unstyled files, all text editing: `text_area.rs`, `text_area/state.rs`,
`text_area/rows.rs`, `text_area/rows/table.rs`, `text_area/hooks.rs`,
`text_area/lines.rs`, `selectable.rs` (`Piece`, `RichLayout`, `RichOptions`,
`CaretHandle`, `handle_center`, `toolbar_anchor`, `HANDLE_RADIUS`). Styled's
`text.rs` uses only the value types `SpanBackground`, `TextCaret`, `TextMark`.
With text editing shared, these go behind the text geometry API and base
`Text`'s props; `selectable.rs` is the native core's selection only.

### 2.5 Tree, document and interaction internals

- `beui_core::tree::component_node`: `dialog.rs`, `drag.rs`, `number_input.rs`.
- `drag_board` (`DragPoint`, `Pending`, `Target`): `drag.rs`, `lib.rs`.
- `interact::autoscroll::AUTOSCROLL_DEAD_ZONE`, `input::TOUCH_DRAG_THRESHOLD`:
  `scroll.rs`, `drag.rs`.
- Timers and animation (`now`, `wall_clock`, `Spring`, `Fling`): 5 unstyled
  files; values plus a frame clock both cores provide.

### 2.6 Outside beui

`beui_core` appears directly in three crates: `be-drive` (`runner::environment`),
`block-ui-test` (`input::typing::chord_name`, `renderer::Renderer`),
`local-tools` (a doc string). None of it is base. The wider leak is
`with_document`, which `beui_view::reactive` re-exports: 11 crate directories
outside `beui-core` and `beui-view` call it (block-app, block-shell,
be-launcher, be-wayland, block-editor-beui, three editors, beui-demo,
beui-inspector), each reaching the native `Document`.

## 3. Focus

Focus is a property of `Interactive` (`focusable`), not its own node.
Patterns in unstyled, with the files that use them:

- **Focus callbacks** (`on_focus_change`, 23 files): `focusin`/`focusout`.
- **Programmatic focus** (`focused=` memo, 8 files; `NodeId::focus()`, 3):
  `element.focus()`. The HTML core must defer it until the element is
  connected, as native already defers through `focus_wanted`.
- **Roving focus** (`tab_stop=` a memo plus `focused=`, 9 files: menu, choice,
  tree, slider, selectable, search list, context menu, pressable, scroll):
  roving `tabindex` 0/-1. `arrow_focus` (styled tree only) walks
  `tab_stops_within`; on the DOM, a query of `[tabindex]` in document order.
  `yields_tab_stop` (scroll only) is a scroller giving up its stop to focusable
  content; browsers already skip a scroller with focusable content.
- **Press focus** (`press_focus`, 5 files): `preventDefault` on `pointerdown`.
- **Traps** (`Overlay traps_focus`, 10 files): `showModal()` or `inert` on
  what is outside. Return focus on close comes from the layer's focus scope
  natively and from `<dialog>` on the web; a `popover` needs the core to
  restore it.
- **Focus ring** (`focus_ring`, `focus_outline`; 1 unstyled, 6 styled):
  `:focus-visible`. Styled computes a `Memo<bool>` and draws its own ring, so
  the HTML core sets that memo from `element.matches(':focus-visible')` on
  focus. It maps.
- **Key routing** (`on_key=` 16 files, `on_key(KeyAt, KeyPhase)` 7):
  `keydown` listeners in capture or bubble phase on the node or the window.
- **IME** (`ime`, `ime_cursor`, `ime_text`: `text_area.rs`,
  `date_time_field.rs`): the hidden `<textarea>`, later EditContext.
- **Focus ancestry** (`focus_ancestry`, command palette): walk
  `document.activeElement`'s parents.
- **Focus scopes** (`push_focus_scope`): no unstyled user; block-app's
  compositor only.

Everything except IME maps onto DOM focus.

## 4. Drift

- **The guide's list omits** `LayoutBuilder`, `PlacementBuilder`, `Root`,
  `Window`, `Spacer`, `Text`'s children (`Span`, `Icon`, `Gap`, `Break`,
  `TextItem`) and the child wrappers (`Layer`, `Fitting`, `GridCell`,
  `CanvasItem`). `base/` holds four internals (§1).
- **Few users in unstyled and styled (≤2 files):** `FirstFit` (1), `Grid` (2),
  `Offset` (2), `Shift` (2), `Turn` (0), `Embed` (0), `Portal` (2),
  `PlacementBuilder` (2), `Window` (0), `Root` (1), `Break` (0), `TextItem` (1).
  `Text` itself has one unstyled user (`text_area/lines.rs`): unstyled takes
  labels as rendered children.
- **Duplicates.** Three transforms (`Shift.by`, `Turn.angle`,
  `Embed.rotation`) and two single-child effect nodes (`Fade`, `Shift`) that
  `Frame` could carry. Five layout queries (§2.2). `Spacer` is an empty `Frame`, kept as the one spelling for fill and empty
children.
  `PlacementBuilder` is `node_rect` as a builder. `Container`/`narrower_than`
  (11 files) is a `LayoutBuilder` that is really a container query.
- **`Interactive` has 61 props.** 16 are used by at most one tag in the
  repository (`yields_tab_stop`, `keyboard_on_focus`, `ime_keyboard`,
  `takes_text`, `on_ime`, `on_motion`, `pointer_locked`, `drawing`,
  `every_sample`, `on_pan_active_change`, `on_autoscroll`, `capture_at`,
  `gestures_at`, `drawing_at`, `inhibits_shortcuts`, `forwards_wheel`). The
  gesture recognisers (pan, zoom, scroll drag, autoscroll, secondary drag,
  touch drag axis) are behaviour that could sit in unstyled over raw pointer
  events.
- **Scrolling is composed in unstyled.** `unstyled::Scroll` (904 lines) builds
  wheel, fling, overscroll and steering from `Offset` and `Interactive`. It has
  55 users. The first version keeps it; the follow-up switch to the browser's
  scroller (compositor-thread scrolling, `Sheet`'s stops as CSS scroll snap)
  happens in unstyled and styled.
- **No image primitive.** `Picture` (18 users) paints images through
  `Drawing`, and `ScrollbarThumb` paints a rounded rect through one. On HTML
  those would be `<canvas>`es for what `<img>` and a `Frame` do.
- **Text editing is built from base text.** `TextInput` (37 users) is a
  `TextArea` with a `password` mask, so a real `<input type=password>` is a
  switch in unstyled. `selectable.rs` (988 lines) builds cross-node selection
  that the browser has natively.
- **Styled re-exports most primitives** (`BackHandler`, `Canvas`, `Drawing`,
  `Embed`, `Fade`, `Interactive`, `Layers`, `LayoutBuilder`, `Offset`,
  `Overlay`, `PlacementBuilder`, `Portal`, `Shift`, `Turn`, `VirtualList`,
  `Window`, ...). So the "base" outside code depends on is that list, and
  `Interactive` (22 files), `Canvas` (22), `Layers` (24), `Drawing` (12) and
  `PlacementBuilder` (9) outside beui fix the API both cores must keep.
- **`beui-view` hands out the native `Document`.** `with_document`,
  `try_with_document` and `enter` are in the prelude, and beui-view's own
  helpers (`focus_ring`, `arrow_focus`, the `TreeNode` methods on `NodeId`) are
  written against `Document`. Those helpers become the core's job; the
  re-export goes.

## 5. Recommendations

### Proposed base set

Shared API, implemented by both cores:

- **Layout:** `Frame` (gaining `translate`, `rotate`, `opacity` and
  `fade_edges` from `Shift`, `Turn.angle` and `Fade`), `List`, `Grid`,
  `Layers`, `Offset`, `VirtualList`, `Canvas`.
- **Text:** `Text` with `Span`, `Icon`, `Gap`, `Break` and inline `TextItem`,
  for display only.
- **Input:** `Interactive`, cut to focus, keys, text and paste, press, click,
  hover, drag, wheel and cursor.
- **Layers:** `Overlay`, `Portal`, `Root`.
- **Measurement:** one late-answer API replacing `LayoutBuilder`,
  `PlacementBuilder`, `node_rect`, `placements`, `node_placed` and
  `text_bounds`: a signal that is `None` until the first layout and then
  follows it. Native fills it in the same frame; HTML from `ResizeObserver`.

`Text` keeps `marks`, `glyphs`, `carets` and `hang`, with semantics pinned
down for both cores, and text geometry joins the measurement API. The
`rich::` internals unstyled uses today go behind those.

Core-implemented components (one API, each core its own implementation):

- `Drawing`: a pink TODO rectangle on HTML at first.
- Read-only text selection: the browser's on HTML.
- `Picture` (`<img>` on HTML), or a base `Image`; pink at first, since it
  paints through `Drawing`.

Moves up or out:

- `Window`: a `beui-view` composition of `Overlay`, as it already nearly is.
- `BackHandler`: a scoped hook, like `on_key`.
- `FirstFit`: unstyled, over the measurement API.
- `Turn` quarter turns: to their users (block-editor-beui, infinite-canvas).
- `Embed`: product-facing, beside the plugin work it serves.
- `Spacer`: stays base for fill and empty children; styled gains a `Spacer`
  taking a `Space` token for fixed gaps, replacing the pixel sizes passed to
  it outside beui.
- Gesture recognisers on `Interactive`: unstyled.
- `Container`/`narrower_than`: stays unstyled, rebuilt on the measurement API.
- `focus.rs`, `focus_scope.rs`, `share.rs`, `child_list.rs`: out of `base/`.

### Decided

1. **Outside code gets a chosen set of primitives**, decided in step 3, not
   everything styled re-exports today. It keeps an escape hatch for what beui
   can't express: a GPU surface to draw on (scene-3d needs a wgpu canvas),
   which on the HTML core is a `<canvas>` with a WebGPU context.
2. **`Embed` and `Turn`'s quarter turns leave beui.**
3. **`Interactive`'s custom hit tests are redesigned** in step 3.
4. **`Break`'s "copies without a newline" is a requirement.** First
   candidate: an empty element whose `::after` is `content: "\A";
   white-space: pre`, since generated content is neither selected nor
   copied. `display: table` on an empty span probably fails, because
   selection serializes block-level boxes with newlines. If neither holds
   in every engine, a `copy` handler writes the clipboard text from the
   model. A Playwright test across Chromium, Firefox and WebKit decides.
5. **`Drawing` as SVG or `<canvas>`**: decided, SVG (see The decision).
6. **The overlay beak moves out of `Frame` into the overlay.** The overlay
   chooses the side it opens on, so it draws the beak there. On the HTML
   core the browser chooses that side through CSS anchor positioning, so the
   overlay follows it with anchored container queries where they exist, or
   by measuring after layout.
7. **`Canvas` is shared base**: a container with
   `transform: translate() scale()` on the HTML core. When zoomed items are
   re-rendered sharp is the browser's choice there, so `raster` is a hint on
   the web.

## Implementation design

What later agents build, in the order of "Tasks" at the end. Names below
are decisions; crate and function names from tasks running alongside this one
(`beui-types` from `value-types`, `beui_html::dom::{mount, hydrate}` from
`html-live`) are used as those tasks land them, and notes.md wins where they
differ.

### Choosing the core

- **A `--cfg beui_html` rustc flag, per buck platform.** Not a cargo feature:
  buck takes features from cargo's plan per platform, and the `wasi` plan
  already unifies block-app's, beui-web-demo's and paint-review-web's, so a
  feature turned on for one would turn on for all. A cfg is also exclusive by
  nature, so `beui-core` drops out of the HTML graph instead of linking unused.
  Two new target platforms in `buck/platforms/BUCK`, each the existing one
  plus a `root//buck/constraints:html` value (setting `beui_core`), the same
  way `wasi_guest` adds `guest`: `wasi_html` (wasm32-wasip1-threads, the
  browser) and `linux_x86_64_html` (the host, for SSR and string tests). Two
  new plans in `buck/cargo/buckify.sh`, `wasi-html` and `linux-x86_64-html`,
  run with `RUSTFLAGS="--cfg beui_html"` over the crates that run on the HTML
  core (the spike, the shared tests and what they pull in). cargo evaluates
  `[target.'cfg(beui_html)'.dependencies]` against RUSTFLAGS, so manifests
  say what each core links; `defs.bzl` adds `--cfg beui_html` to every
  first-party rustc under those plans; the workspace's `unexpected_cfgs`
  lint gets `check-cfg = ["cfg(beui_html)"]`; `//:check` checks the HTML
  configurations too. If cargo's plans turn out to ignore the cfg, fall back
  to a `beui-view/html` feature in the separate plans, with
  `default-features = false` on every `beui-view` dependency.
- **One constant.** `beui_view::CORE: Core` (`enum Core { Native, Html }`),
  `if cfg!(beui_html) { Core::Html } else { Core::Native }`, re-exported as
  `beui::CORE`. Unstyled and styled switch only with `if CORE == Core::Html`,
  so both branches compile on both cores and no feature exists on only one.
  `#[cfg(beui_html)]` is allowed only in `beui-view`, `beui`, `beui-core-html`
  and buck files; `fix-rust-source` gets a lint rejecting it elsewhere, like
  its existing "document use in a component" lint.
- **beui-view** keeps one signature per primitive: each `#[component]` in
  `src/components/*.rs` declares the props once and its body calls
  `crate::core::<name>(..props)`. `src/native/` (today's bodies, against
  `beui-core`'s `Document`) and `src/html/` (against `beui-core-html`) each
  implement those functions; `lib.rs` picks one as `mod core` by cfg. Hooks
  (`on_key`, `focus_ring`, `holds_focus`, `arrow_focus`, `copy_text`,
  `use_viewport_size`, `use_preferences`, timers, the `TreeNode` methods) and
  the measurement API dispatch the same way. Building the HTML configurations
  in `//:check` is what keeps the two bodies' arguments in step.
- **The `beui` facade.** Native: as today, except that `Document`,
  `with_document`, `try_with_document` and `enter` move from
  `beui_view::reactive` to a `beui::native` module (with `Embed`), so code
  that reaches the native document is visibly native and fails to build on
  HTML. HTML: `reactive`, `styled`, `unstyled`, `datetime`, `icons`,
  `primitives`, `beui-types`' values, `CORE`, and `beui::html::{run, hydrate,
  render_to_string}`; renderers, platforms, dock, inspector, keyboard and
  layers are `cfg(not(beui_html))` dependencies.

### Handles: `NodeId` and `Child`

`NodeId` (index and generation) is a plain value in `beui-types`, and both
cores hand them out from their own arenas, so the 454 lines naming `NodeId` in
unstyled and styled and `beui_view::reactive::Child = NodeId` stay as they
are, `Copy`, `Eq` and `Hash` on both. `NodeOf<T>` stays native-internal.
`#[component]` and `view!` are unchanged: the macros already name
`::beui_view::reactive::Child` (or `beui::reactive::Child`), `IntoChild`,
`Children<ListChild>` and `SlotHost` from `beui-tree`, which knows nothing of
either core. Each core implements `SlotHost<ListChild>` (and the other child
types) for its containers.

### The HTML core: `beui-core-html`

A new crate beside `beui-core`, depending on `beui-types`, `beui-tree`,
`reactive`, `beui-html` (with its live backend on wasm) and, on wasm only,
`web-sys` and `beui-web-input`. Its document is a thread-local arena:
`NodeId` to an entry holding the `beui_html::Node`, the primitive's kind, its
measurement signals, focus state, test id and accessibility. Every primitive
is one element; nothing wraps a child except where a row below says so.
Values become inline `style` properties (`Extent`, `Sides`, `Align`,
`Color32`, `Corners`, `Outline` each get one `to_css`); shared classes in a
stylesheet are a later size optimisation.

| Primitive | Element and CSS |
| --- | --- |
| `Frame` | `div`, `display: grid` with the child in one cell; `width`/`height` from `Extent` (min, max, fill as `flex-grow` from the parent's side), `aspect-ratio`, `padding`, `place-items` from `align`, `background`, `border-radius`, `outline`/`outline-offset`, and the new props `translate`/`rotate` (`transform`), `opacity`, `fade_edges` (`mask-image: linear-gradient`) |
| `List`, `Spacer` | `div`, `display: flex` (or `flex-wrap`), `gap`, `align-items` incl. `baseline`, `justify-content`; a child's `@sizing` becomes `flex` on that child's element. `Spacer` is an empty `div` |
| `Grid` / `GridCell` | `display: grid`, `grid-template-columns` from `Track`s (px, `max-content`, `fr`), `grid-column: span n` |
| `Layers` / `Layer` | `display: grid`, each child at `grid-area: 1 / 1` |
| `Offset` | `overflow: hidden; overflow-anchor: none`, moved by `scrollTop`/`scrollLeft`; anchoring, `follow_end` and `reveal` are beui's, computed from the measurement API as native does |
| `VirtualList` | flow layout between two spacer `div`s; builds the rows that the parent `Offset`'s position reaches plus overscan; row sizes from `measure`, remembered per key; anchoring corrects the `Offset` in the same frame |
| `Canvas` / `CanvasItem` | `position: relative; overflow: hidden`, an inner `div` with `transform: translate() scale()`, items `position: absolute`; `raster` is ignored, `on_reveal` is late |
| `Text` | `div` with `white-space: pre-wrap`, `overflow-wrap: anywhere`, `line-clamp`/`text-overflow` from `Overflow`, `line-height`, `tab-size`; `hang` as negative `text-indent` with `padding-left`; `marks` as CSS Custom Highlights (one `Highlight` per mark colour, `::highlight(beN)` rules); `glyphs` as `span`s replacing their range; `carets` as absolutely placed `div`s at a `Range`'s rect |
| `Span`, `Icon`, `Gap`, `Break`, `TextItem` | `span` (`box-decoration-break: clone`, `break_after` as `wbr`); `span aria-hidden` in the icon font with `user-select: none`; `inline-block` with a width; the decided `::after { content: "\A" }` element; `inline-block` around the child |
| `Interactive` | listeners and `tabindex` on its child's element; its own `div` only when it has no child (below) |
| `Overlay` | `popover="manual"` (`<dialog>` with `showModal()` for `BlocksAll` and traps), anchored by CSS anchor positioning (`anchor-name` on the anchor, `position-area` from `Placement`, `position-try-fallbacks`), a point or rect anchor as an invisible absolutely placed `div`; scrim on `::backdrop`; the beak follows the side the browser chose, read back from `measure` |
| `Portal` | moves its target's element with `moveBefore`, else `insertBefore` |
| `Root` | the mount element; body background, `document.title`, `<meta name="theme-color">`, `color-scheme` |
| `Drawing` | a `div` of its size holding an inline `<svg>` that the `Draw` callback paints through the HTML `Painter`, redrawn when the callback or the measured size changes (guides/beui.md, Cores) |

Until a primitive's task lands, its HTML body renders its children in a plain
`div` with a pink dashed outline (a leaf: a pink box), so a screen built from
styled is readable from the first task on.

From `beui-html` the core uses `Node` (element and text handles, live on
wasm and strings on the host), attributes and inline style properties, child
insertion through `SlotHost<Node>`, event listeners, and hydration's
claim-in-order cursor. Hydration needs the browser's first build to create
the server's elements in the same order: measurement answers, focus and
anything else the server cannot know start as `None` on both sides and
change only after `hydrate` returns.

### Measurement

One API in `beui-view`, the same on both cores:

```rust
pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>>;
pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>>;

pub struct Measured {         // beui-types
    pub rect: Rect,           // window space: getBoundingClientRect on the HTML core
    pub size: Vec2,           // laid-out size before transforms: offsetWidth/offsetHeight
}

impl TextGeometry {           // Clone; each core's own type, same methods
    pub fn bounds(&self) -> Rect;
    pub fn caret_rect(&self, index: usize, width: f32) -> Option<Rect>;
    pub fn index_at(&self, pos: Pos2) -> Option<usize>;
    pub fn inline_at(&self, pos: Pos2) -> Option<usize>;
    pub fn lines(&self) -> Vec<LineBox>; // LineBox { bytes: Range<usize>, rect: Rect, baseline: f32 }
}
```

- `None` until the node is first laid out, and again when it is unplaced.
  Rects are logical pixels in the window's space, the space `PointerPress`
  positions use; `Measured::size` is the node's own laid-out size, unscaled by
  any zoom, which is what content built from its own size reads. Each node has at most one signal per function, made on first
  call and dropped with the node, so a handler may call
  `measure(node).get_untracked()` as often as it likes.
- It replaces `LayoutBuilder` (`measure` of a filling `Frame`, then `Show`
  on the size), `PlacementBuilder` and `node_rect` (`measure`), `node_placed`
  (`is_some`), `placements().rect` (`get_untracked`), `text_bounds` and the
  `TreeNode` text methods (`measure_text`), and `layout_text`/`rich_layout`
  for editors, which ask a rendered `Text` instead. `viewport_rect` goes;
  `use_viewport_size` stays.
- Offsets are UTF-8 bytes of the `Text`'s content. A `glyphs` replacement
  counts as the bytes it replaces; `Icon`, `Gap` and `TextItem` as zero. The
  HTML core maps them to UTF-16 offsets per DOM text node.
- **Native** fills both in `Document::report_laid_out`, inside the existing
  loop of `update_layout` (up to `LAYOUTS_PER_FRAME` layouts), so an effect
  that reacts by changing the tree is laid out again before the frame paints:
  same frame, as `LayoutBuilder` was. `TextGeometry` wraps today's
  `RichLayout` and `Rects`.
- **HTML** fills them in the frame loop's measure pass (below):
  `getBoundingClientRect` minus the root's origin, and `TextGeometry` queries
  `caretPositionFromPoint` and `Range.getClientRects()` synchronously.
- **Delayed measurement for tests:** `beui::testing::delay_measurement(true)`,
  or `BEUI_DELAY_MEASUREMENT=1` for a driven program, makes native publish
  measurements after the frame paints and request another frame, so headless
  tests take the late path the HTML core can take.

### Running

- **Entry points:** `beui::html::run(element_id, root)` builds into an empty
  element, `hydrate(element_id, root)` adopts server-rendered HTML, and
  `render_to_string(root) -> String` renders on the host (no layout, no
  listeners).
- **Frame loop.** Signal changes write the DOM at once from their bindings;
  nothing is diffed. A frame (`requestAnimationFrame`) is scheduled only when
  something asks: a timer or animation due, a DOM change while a node is
  measured, a `ResizeObserver` callback on a measured node or the root, a
  scroll, a font load. A frame: advance the clock, run due timers and
  animations, flush effects, then the measure pass: read the rects of every
  measured node whose subtree changed (reads only, one forced layout), set
  their signals, flush; repeat up to `LAYOUTS_PER_FRAME` times, as native
  does. rAF runs before style, layout and paint, so a settled measure pass is
  never seen half done.
- **Input.** Listeners on the element `Interactive` decorates:
  `pointerdown/move/up/cancel`, `pointerenter/leave` (hover), `click`,
  `auxclick` (middle), `contextmenu` (secondary press), `keydown/keyup`,
  `focusin/focusout`, `wheel` (not passive), `paste`. Drag is pointer capture
  after `TOUCH_DRAG_THRESHOLD`; `cursor` is `style.cursor`; `press_focus=false`
  is `preventDefault` on `pointerdown`. `on_key(KeyAt, KeyPhase, ..)`
  listens on the node's element, the root element or `window`, in capture or
  bubble phase. Key and wheel values come from `beui-web-input`, a new crate
  that takes `keys.rs`, `wheel.rs` and `ime_mirror.rs` out of
  `beui-platform-web` and `beui-core::app`, depending only on `beui-types`
  and `web-sys`, so both web platforms share them.
- **Focus** is the browser's: `focusable` sets `tabindex` (0, or -1 when
  `tab_stop` is false, which gives roving focus), `focused=` and
  `NodeId::focus()` call `element.focus()` once the element is connected,
  `focus_ring` reads `:focus-visible` on focus, `focus_ancestry` walks
  `document.activeElement`, traps are `showModal()` or `inert`, and a
  `popover` overlay restores focus on close itself.
- **IME.** One hidden `<textarea>` agent, styled as `beui-platform-web`'s is.
  When an `Interactive` with `ime` or `on_text` takes focus, the core moves
  the agent into that element, places it at `ime_cursor` and focuses it, so
  `focusin` and `:focus-within` still reach the element. `ImeMirror` turns
  the agent's `input` and `composition*` events into `on_text` and `on_ime`,
  and `ime_text` keeps the agent's value and selection in step.
- **Hit tests.** Until their redesign, `hit_at`, `claim_at` and the rest are
  asked in the element's own listener, and a refusal lets the event bubble.
  The redesign's default is declarative hit regions, which on HTML are
  `pointer-events: auto` children of a `pointer-events: none` element.
- **Time.** `now()` is the frame clock: `performance.now()` at the start of
  each frame and of each event dispatch, frozen by `drive clock stop`.
  `create_timer` uses `setTimeout` to schedule a frame; animations request
  frames while they run. `Spring`, `Fling` and easing are values and move to
  `beui-types`; the clock and scheduler are each core's.
- **Fonts.** The bundled faces and the icon font are files beside the bundle,
  named in an `@font-face` sheet the core puts in `<head>` (and the SSR
  page). `FontId` becomes the `font` shorthand. `document.fonts` loading bumps
  `use_font_revision` and schedules a measure pass.
- **Theme.** `prefers-color-scheme` and `prefers-reduced-motion` feed the
  system preference layer as `beui-platform-web` does; styled resolves tokens
  to concrete colours, which reach elements as inline styles; `Root` sets the
  page's background and `color-scheme`.
- **Selection** of display text is the browser's (`user-select: text`);
  `beui-view`'s `selection.rs` is native only. Copy is the browser's too.

### Testing

- **Unchanged:** `beui-tree`, `beui-html`, `reactive`, `beui-types` and
  logic-only unstyled tests (`text-editor-core`, datetime, fling, slider
  scales) are core-independent. Paint tests stay native.
- **HTML paintings.** `beui-core-html`'s host tests render each primitive with
  `render_to_string` into a `paint_snapshot::BrowserSnapshot`
  (`<name>.browser`), with the font sheet as a resource, as be-server's portal
  tests do; that is the painting that demonstrates an HTML change.
- **Shared behaviour tests.** A crate `beui-shared-tests` holds fixture
  screens (one function per screen, built for both cores) and `drive/*.drive`
  scripts restricted to the commands both cores answer: `settle`, `pause`,
  `clock stop|run`, and `app tree|ids|find|wait|gone|click|type|key`.
  - Native: a Rust test per script runs it through `be_drive::Driven`
    against the crate's headless `:dev` program, once normally and once with
    `BEUI_DELAY_MEASUREMENT=1`.
  - HTML: `beui-core-html` exports `automate(words)` (the page's `automate`,
    as `?automation` already means for block-app): `tree` and `ids` from its
    arena and `getBoundingClientRect`, in native's line format; `find TARGET`
    answers a rect; `wait`/`gone` poll each frame; `settle` waits for no
    frame pending. `buck/app/web-test.mjs`, sharing its command parsing with
    `web-drive.mjs`, serves the bundle, opens it in Chromium through
    Playwright, sends the read commands to `automate`, and performs `click`,
    `type` and `key` as real Playwright input at the rect `find` answered, so
    the browser's own focus, IME and event paths run.
  - The HTML run is a buck test (`web_drive_test` in `buck/app/drive.bzl`)
    run locally, since it needs a browser: `BE3_CHROME` when set (CI's
    runner Chrome, as the rasterizer uses), else Playwright's Chromium. It
    fails, never skips, when neither is there. If the workers' image gains
    Chromium, it moves to the build server.
  - Scripts match names, never coordinates, since text metrics differ.

### The spike

The screen is block-app's account picker, `AccountsScreen` in
`crates/block-app/src/ui/onboarding.rs` ("Choose an account to continue.",
account cards, "Add account"): the first screen a person signs in through,
small, and still a real mix of `Scroll`, `Card`, `Button`, `MenuButton`,
`Tooltip`, `Icon` and `ForEach`.

- It moves, with `Column`, `ErrorText`, `AccountCard`, `AccountDetails` and
  `AccountRow`, into a new crate `block-accounts-ui`, taking
  `accounts: Memo<Vec<AccountRow>>`, `error: Memo<Option<String>>` and
  callbacks (`on_add`, `on_open`, `on_choose_organization`, `on_log_out`)
  instead of `AppViewStore` and `send`. block-app passes those; a native
  painting test in the new crate keeps it from regressing.
- `crates/beui-html-spike` builds it with two fixture accounts; `on_add`
  appends a third, which shows hydration left it live. Its `:ssr` binary
  (for `linux_x86_64_html`) writes `index.html` with `render_to_string`, a
  genrule runs it at build time, and the wasm module (for `wasi_html`) calls
  `beui::html::hydrate` on load.
- `./scripts/buck run //crates/beui-html-spike:web` serves it with Caddy on
  http://127.0.0.1:8060 (beui-web-demo's `app(...)`, shims and Caddyfile
  pattern); `?automation` answers `drive` once task 8 lands.

### Tasks

Phase 1 can start once `value-types`, `remove-dom-renderer` and `html-live`
have landed; its tasks run in parallel unless marked.

1. **Core selection.** The `html` constraint, `wasi_html` and
   `linux_x86_64_html` platforms, the two plans, the cfg flag, check-cfg,
   `//:check` over the HTML configurations, `beui_view::CORE`, the
   `fix-rust-source` lint; `beui-view` split into `components/` signatures
   with `native/` bodies (no HTML bodies yet). Files: `buck/constraints/BUCK`,
   `buck/platforms/{BUCK,cross.bzl}`, `buck/cargo/{buckify.sh,defs.bzl,crates.bzl}`,
   `buck/dev/workspace.bxl`, `Cargo.toml`, `crates/beui-view/src/**`,
   `crates/fix-rust-source/src/**`, guides/buck2.md.
2. **Frame and layer cleanups.** `Frame` gains `translate`, `rotate`,
   `opacity`, `fade_edges`; `Shift` and `Fade` go and `Turn` keeps only
   quarter turns; the beak moves from `Frame` into the overlay; `Window`
   becomes a `beui-view` composition of `Overlay`; `BackHandler` becomes an
   `on_back(handler)` hook; styled gains `Spacer` taking a `Space`; `Embed`
   moves to `beui::native`. Files: `crates/beui-view/src/components/{frame,shift,fade,turn,overlay,window,back,embed}.rs`,
   `crates/beui-core/src/base/{frame,shift,fade,turn,overlay,back,embed}.rs`,
   their users in unstyled, styled, beui-demo, block-app, block-editor-beui,
   editors.
3. **Measurement API (native).** `measure`, `measure_text`, `TextGeometry`
   with `lines`, delayed measurement; every caller in §2.2 moved to it;
   `LayoutBuilder`, `PlacementBuilder`, `node_rect`, `node_placed`,
   `placements`, `text_bounds` deleted; `FirstFit` and `Container` rebuilt
   in unstyled; `Turn`'s quarter turns moved to their users as `Frame` plus
   `measure` (after task 2, which touches `turn.rs` first). Files:
   `crates/beui-view/src/{nodes.rs,reactive.rs,measure.rs}`,
   `components/{layout_builder,first_fit,turn}.rs`,
   `crates/beui-core/src/{document.rs,base/text.rs}`, unstyled's `sheet`,
   `text_area*`, `selectable`, `context_menu`, `scroll`, `tree`,
   `back_slide`, `dialog`, `container`, `date_time_picker`, `pan_zoom`,
   styled's `choice.rs`, and the outside `PlacementBuilder` users.
4. **Core internals out of view.** `focus.rs`, `focus_scope.rs`, `share.rs`
   and `child_list.rs` leave `beui-core/src/base/`; `with_document`,
   `try_with_document`, `enter` and `Document` leave `beui_view::reactive`
   for `beui::native`, and the eleven crates calling them import it from
   there (or get a `beui-view` API where one is plainly missing). Files:
   `crates/beui-core/src/{base,base.rs,focus*,layout*}`,
   `crates/beui-view/src/{reactive.rs,nodes.rs}`, `crates/beui/src/lib.rs`,
   block-app, block-shell, be-launcher, be-wayland, block-editor-beui,
   editors, beui-demo, beui-inspector.
5. **Interactive.** Gesture recognisers (pan, zoom, scroll drag, autoscroll,
   secondary drag, touch drag axis) move to unstyled hooks over new raw
   `on_pointer` and `on_wheel` props; hit tests become declarative regions;
   props with one user are folded or moved. Files:
   `crates/beui-view/src/components/interactive.rs`,
   `crates/beui-core/src/{base/interactive.rs,interact/**}`, unstyled's
   `scroll`, `pan_zoom`, `drag`, `slider`, a new `gestures.rs`, and the
   outside users of the props it removes. After task 4 (`focus.rs`).
6. **HTML core skeleton** (after 1–5). `crates/beui-core-html`: arena,
   frame loop, `run`/`hydrate`/`render_to_string`, measurement, focus,
   input, IME agent, fonts, theme; `crates/beui-web-input` from
   `beui-platform-web` and `ime_mirror`; `beui-view/src/html/` bodies for
   `Frame`, `List`, `Spacer`, `Grid`, `Layers`, `Text` (display text and its
   inline children, no `marks`/`glyphs`/`carets` yet), `Interactive`, a basic
   `Offset`, `Root`, pink `Drawing`, outlined stand-ins for the rest; the
   `beui` facade under HTML; BrowserSnapshot tests per primitive.
7. **Spike** (after 6; parallel with 8–10). `block-accounts-ui`,
   `crates/beui-html-spike` with `:ssr` and `:web`, block-app switched to the
   extracted screen.
8. **Shared behaviour tests** (after 6; parallel with 7, 9, 10).
   `automate` in `beui-core-html`, `buck/app/web-test.mjs`,
   `web_drive_test` in `buck/app/drive.bzl`, `crates/beui-shared-tests` with
   fixtures for `Frame`, `List`, `Text`, focus and `Interactive`, the native
   runs under `be_drive::Driven`, CI's workflow installing Playwright, and
   guides/running/web.md and guides/testing_a_gui.md.
9. **HTML containers** (after 6). `Offset`'s anchoring, `follow_end` and
   `reveal`; `VirtualList`; `Canvas`; `Overlay` with anchor positioning,
   scrim, traps and the beak; `Portal`. A shared script for each in
   `beui-shared-tests` once 8 lands.
10. **HTML text editing** (after 6 and 3). `Text`'s `marks`, `glyphs`,
    `carets`, `hang`, `TextGeometry` from `Range`, the UTF-8 to UTF-16 map,
    `Break`'s copy test across Chromium, Firefox and WebKit; `TextArea` and
    `TextInput`'s own layout (`lay_out_prose`, `galley`) moved onto
    `measure_text`, natively first.
11. **Port unstyled and styled** (after 8–10). A shared script per unstyled
    component, bottom-up, fixing what each finds on either core; `TextArea`
    last; beui-demo's pages on the HTML core as `//crates/beui-demo:web-html`.
12. **Guides.** A "Cores" section in guides/beui.md (the contract, `CORE`,
    the measurement API, what each primitive is in HTML, how to test on both),
    the "Component layers" table and base list brought up to date, and
    VISION.md's open question answered: two cores, with the browser laying
    out the web. Each earlier task updates the guide it touches; this one
    writes the overview, after 11.

## Left

- Browser-native text editing: EditContext (or `contenteditable`) for
  `TextArea` and `TextInput` on the HTML core, and `TextInput`'s `password`
  mode as `<input type=password>`, each a switch in unstyled.
- A `Drawing` with no `size` draws nothing in a server-rendered page, since
  the HTML core measures nothing there; so the scrollbar thumb and the colour
  picker's areas and sliders appear only once the page runs.
- `Scroll` switching to a real `overflow: auto` scroller on the HTML core.
- `TreeNode`'s `row_offset`, `texts_within`, `set_text_selection` and
  `set_text_selection_handles` do nothing on HTML.
- Props the HTML core ignores: `Interactive`'s `pastes`, `key_active`,
  `claims_touch` and the forwarding group (`on_forward`, `inhibits_shortcuts`,
  `clipboard_keys`, `forwards_wheel`); `Overlay`'s `window` and `shown_in`;
  `Root`'s `system_bars`; `Canvas`'s `scrolls_across`; `VirtualList`'s `text`;
  `Selectable`'s colours, handles and toolbar. A `TextItem` with `at` is a
  stand-in, `on_back` never fires and `use_keyboard()` is always true.
- `Break`'s copy test across Firefox and WebKit (§5, Decided 4): the shared
  scripts run only in Chromium.
- Porting block-app itself to the HTML core; it runs the native core with wgpu
  on the web.
- Web plugins as iframes, which could run the HTML core.
- `color_var!` is meant to be the only way to make a `ColorVar`, but
  `ColorVar::__declared_at`, which it expands to, is public (doc-hidden), so
  any code can name one by string.
- `beui-font-browser` is still unused.
- HTML text uses the browser's font metrics, which round a font's ascent and
  descent to the nearest pixel where FreeType rounds them outward, so a line
  can sit a pixel off native (a 20px title, icons).
