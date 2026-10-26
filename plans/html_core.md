# An HTML core for beui on the web

beui on the web gets a second core. This file is step 1: an inventory of the
base layer both cores would have to implement, and a proposal for what that
layer should be.

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
- **Escape hatches are pink in the first version.** `Drawing`, and what
  paints through it (`Picture`, the scrollbar thumb), draw a pink TODO
  rectangle of the right size on the HTML core. Follow-ups give each a real
  implementation: an `<img>` or an image primitive for pictures, a `Frame`
  for the thumb, and `<canvas>` or SVG for drawings (open question 6).
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
  browser on the HTML core, the same split `beui-renderer-dom` already lives
  with through `beui-font-browser`; the editor only asks for geometry and
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
| `Frame` | one child: size (`Extent`), aspect ratio, padding, align, fill, radius, outline, overlay `pointer` beak | 12 | 3 | 6 | 0* | CSS: box, `aspect-ratio`, `padding`, `place-self`, `background`, `border-radius`, `outline`+`outline-offset`. The beak follows the side the overlay chose: **unclear** (needs the chosen anchor fallback) | Absorb `Shift`, `Turn.angle`, `Fade` as props |
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

`beui_core` appears directly in four crates: `be-drive` (`runner::environment`),
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

### Open questions

1. Should outside code keep reaching primitives through styled's re-exports,
   or should step 3 decide which of them (`Interactive`, `Canvas`, `Layers`,
   `Drawing`) outside code may use?
2. Do `Embed` and `Turn`'s quarter turns leave beui, given that no unstyled or
   styled component uses them?
3. `Interactive`'s custom hit tests (`hit_at`, `capture_at`, `gestures_at`,
   `drawing_at`, `claim_at`) have no DOM equivalent short of re-dispatching
   through `elementsFromPoint`. Keep them, or redesign them in step 3?
4. `Frame`'s `pointer` beak follows the side the overlay chose. On the HTML
   core that needs the chosen anchor fallback (anchored container queries).
   Keep the beak, or move it into the overlay?
5. Is `Break`'s "copies without a newline" a requirement on the web, where
   only the terminal's soft-wrapped rows depend on it?
6. Should `Drawing` become SVG or `<canvas>` on the HTML core? SVG is crisp
   at any zoom and stays in the DOM, but a drawing that redraws often (a plot
   while panning, anything culled to its clip) rebuilds many elements each
   time; `<canvas>` is cheap to redraw but blurry until re-rendered at the
   new scale. Undecided; the first version draws pink TODO rectangles.
7. Is `Canvas` shared base (CSS transforms) or core-implemented, given its
   raster settling and scaled spaces have no CSS counterpart to test against?

## Next steps

2. Move shared value types (`NodeId`, `Align`, `Direction`, `TextAlign`,
   `Overflow`, `Placement`, `Key`, `LayerPolicy`, ...) into a core-independent
   crate, and drop `beui-core` from unstyled's and styled's deps so buck
   enforces the boundary.
3. Redesign the real leaks: layout queries, `Painter` outside `Drawing`,
   `rich::` internals, focus.
4. Spike the HTML core (`Frame`, `List`, `Text`, `Interactive`, hydration) on
   one sign-in-style screen.
5. Shared drive-script tests across native headless and the HTML core under
   Playwright.
6. Port unstyled bottom-up; `TextArea` last.
7. Follow-ups: real implementations for the pink escape hatches; unstyled
   and styled switching `Scroll` to a real scroller and `TextInput`'s
   `password` mode to `<input type=password>`; EditContext for text input.
8. A "cores" section in guides/beui.md, and settle VISION's open question.
