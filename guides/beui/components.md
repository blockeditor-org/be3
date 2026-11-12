# Beui components

The unstyled and styled component layers, and the base nodes they compose. Text, text areas, fonts and icons are in [text.md](text.md); overlays, menus' layers, dialogs, toasts and trees in [overlays.md](overlays.md); writing views and the base layer's internals in [core.md](core.md).

## Layers

| Layer | Crate, public path | Owns |
| --- | --- | --- |
| Base | `crates/beui-view/src/components` (bodies in `src/native/` and `src/html/`); `beui::primitives` | Layout, painting, focus, pointer input, offset content, text, per core |
| Unstyled | `crates/beui-components-unstyled`; `beui::unstyled` | Interaction, keyboard, touch and accessibility, with no appearance |
| Styled | `crates/beui-components-styled`; `beui::styled` | Themed controls: an unstyled control plus a face painted from `styled::theme` tokens |

Work at the highest layer that can express the change. For a new look on existing behaviour, paint the unstyled component's handle: a styled control that reimplements focus, keys or pointer handling drifts from every other face of that control. New behaviour is a new unstyled component; a new base node is only for what no arrangement of nodes expresses ([decision](../../decisions/beui_base_layer_kept_small.md)).

Each crate's `lib.rs` re-exports are the list of what exists. Code outside beui reaches the unstyled components that draw nothing (`Draggable`, `DropTarget`, `Floating`, `PointerLock`) through `beui::styled`.

`TextArea`, `TextInput`, `Picture` and `Window` are compositions, the same on both cores; `Selectable` is the one component each core builds its own way ([text.md](text.md#selecting-text)).

## Base nodes: traps

[core.md](core.md) covers the base layer. What trips component code:

- `List` is the only row and column; there are no `Row`/`Column` aliases ([decision](../../decisions/beui_list_props_over_row_column_aliases.md)). A shrinking or growing child never gets less than its `Element::min_width`.
- A `Frame`'s `max` caps the room it takes, not its content (CSS `max-width` on a block): `Extent::at_most(320.0)` is 320 wide however little its content needs. A box as wide as its content up to a cap is a list child in `<ListItem max=..>`. A rotated frame takes input unturned, so `rotate` suits pictures, not controls.
- Nothing has a hidden state while in the tree: use `Show` (`keep_alive` to keep state), inside a `Layers` in a one-child slot.
- A node no `Portal` shows keeps the arena parent of the last portal that showed it, so code walking up checks each parent still lists it.

## Tokens and theme

Styled components take design tokens from `styled::tokens` (`Space`, `Padding`, `Size`, `Length`, `ColorRole`, `TextColor`, `Radius`, `Border`, `Stroke`; [decision](../../decisions/beui_styled_components_take_design_tokens.md)). A raw number in a styled prop does not compile, and the source lint rejects `Extent::px` outside beui's crates. `Custom` variants are for values from content (a user's tag colour, a canvas). `Space` and `Size` resolve through `use_density()`; `use_space` and `use_size` give the pixels an unstyled prop takes. Converting pixels, snap to whichever neighbouring step looks right.

Containers use two fills: `Background` for what everything sits on, and `Surface` for every panel above it, floating ones included (bars, popovers, menus, tooltips, dialogs, toasts). A floating panel stands apart from what it covers by its `Border::DEFAULT`, not by a third fill. `Control` fills what you press, type into or drag (fields, unchecked boxes, chips, tiles, keys, cards you pick or move), so a control shows up the same way on any panel. `Shade` tints an area inside a panel that is not a control: code, a header strip, a highlighted lane.

Colours on primitives are `beui::Color`: a literal, or a reference to a `ColorVar` (`beui::color_var!`) that whoever uses it defines per scheme with `define_colors`. The HTML core writes references as CSS variables with light and dark definitions, so a prerendered page shows the browser's scheme before any script runs. Code that picks a colour by reading the theme kind or scheme renders a prerendered page wrong, since server and browser can disagree; pick a reference whose definitions differ per scheme ([decision](../../decisions/beui_colours_are_scheme_references.md)), and call `resolve_color` only where a `Color32` is needed (a `Drawing`).

`styled::Theme` (`theme.rs`) is the one place theme colours live. Read it with `use_theme()`, one signal per token, so a component wakes only for colours it uses. `ThemeProvider` gives a subtree literal colours; `define_themed_colors` and `theme.themed(..)` give a caller's own per-theme colours the same scheme treatment.

## Controls

- **Buttons.** `unstyled::Button` owns semantics, activation, disabled state and accessibility, and hands its face a `ButtonHandle`; a button that opens something is a `PopupTrigger`. A read-only editor binds `disabled` to `editor.read_only()` rather than leaving a live control that drops edits. Icon-only buttons need no tooltip wrapper: `icon_only` turns the label into the tooltip.
- **Rows and tiles.** `RowFace` is the one pressable-row look (list, action, settings, menu-sheet and palette rows); `TileButton` picks among pictures, `SwatchButton` among colours. `ToolPill` is the bar floating tools sit in. `keep_clear(node, rects)` names parts of a node something floats over, so revealing focused content avoids them.
- **Anchored popups.** Callers never choose dropdown or sheet: `unstyled::Anchored` opens a dropdown for a mouse and a `ModalSheet` for a finger ([decision](../../decisions/beui_anchored_popups_sheet_on_finger_touch.md)). Menus, `Select`, `Popover` and the pickers sit on it; every component with a menu takes a required `MenuStyle` for both looks. `MenuNodes` builds rows from a description of actions (guides/beui_keyboard.md, Actions and the command palette), so one description fills every menu. Popup content focuses by binding `focused` to the handle's `shown`, not to `open`.
- **Sheets** scroll their own content; a vertical `Scroll` inside one measures nothing tall.
- **Dates and colours.** `beui::datetime` values carry no zone (`DateTime::from_unix` reads UTC seconds, as block content stores). Colour controls, like `NumberInput`'s scrub, report a drag through `on_preview` and write `value` once at the end, so undo gets one step per gesture. `SliderScale::Midpoint` curves a slider ([decision](../../decisions/beui_slider_midpoint_exponential_curve.md)).
- **Treemaps.** `Treemap` shows one level of sized items as squarified tiles (an area per size) and goes into a tile that `opens`; the caller holds the `path` and derives `items` from it, so a level is listed only once it is gone into, and a tile's size is the item's own data, never a sum of what is below. A level past `TREEMAP_MOST_TILES` shows the largest and one tile for the rest, so it lays out in time linear in its items.
- `component_placed()` tells a component whether it is laid out, so work like a progress sweep stops while hidden.

## Scrolling

Program code wants `styled::Scroll`, which adds the overlay scrollbar ([decision](../../decisions/beui_overlay_scrollbars.md)) and edge fades. `unstyled::Scroll` owns input and momentum and takes a `ScrollbarStyle`; a control that scrolls something of its own takes one and passes it down (as `Select` does). Under both is the base `Offset`, which answers no input and never clamps.

- Padding goes inside a scroll because a focus ring needs room at the content's edges; a scroll that is part of a padded surface takes `bleed` instead.
- Content is laid out at least as long as the scroll, and content that overflows is first shrunk to `Element::least_length` (shrinking children give up length), like a flex box in `overflow: auto`. A scroll sized to its content up to a bound is `max_length`; a `Frame` with a `max` around it fills up to the max instead.
- Scrolls chain as in a browser: one at its edge when a gesture starts turns it down, so the enclosing one takes it. A horizontal scroll leaves a plain wheel to what is around it.
- The offset is anchored to the node at the viewport's top, so content above it changing size leaves what is read in place. `follow_end` keeps a log or terminal at its end while it grows.
- On the HTML core `unstyled::Scroll` is a real `overflow: auto` element through the platform escape (`BrowserScroll` in `scroll.rs`): the browser scrolls and chains it, with none of beui's momentum, rubber band or drawn bar.

### Long lists

Layout culls nodes more than `CULLING_MARGIN` outside the view (the focused node never), so a long scroll of ordinary content needs nothing more. A `VirtualList` is for when building every row is itself the cost.

It can sit anywhere a tall child could. Its trap is recycling: the row builder runs once per built row with a `ReadSignal` of its key, which the list later points at another key. Derive everything shown from that signal; state a row keeps carries over to the next key, unless its content is in `<Dynamic value={key}>`.

## Inputs and forms

On the HTML core a plain single-line `TextInput` is a real `<input>`, so the browser gives it IME, autofill and password managers; every other text field is beui's own on both cores ([decision](../../decisions/beui_html_text_input_is_a_native_input.md)). A new kind of single-line field is an `InputKind` variant. `Form` is a `<form method="post">` on HTML, so a server-rendered page posts before wasm loads; natively it is no node. Fields beui draws itself (`Checkbox`, `Switch`, `Select`, a beui-drawn `TextInput`) post under their `name` through a hidden input: a checked box `on`, an unchecked one nothing, a `Select` its option's `value` (its label without one), and a disabled field nothing, as in HTML.

The platform escape, `beui::escape::html_escape`, puts a browser element into the tree. It panics natively, so it goes under `if CORE == Core::Html` in unstyled or styled, with a beui version in the other branch behind the same API. It is only for components whose HTML form is the browser's own element (a field, form, link or scroller), not for working around a primitive a core lacks ([decision](../../decisions/beui_platform_escape_only_for_browser_elements.md)).

- **Files.** `create_file_picker(picked)`: nothing blocks; the runner hands the request to `Platform::pick_file`, and the answer arrives in a later frame's reactive scope.
- **Pointer lock.** `unstyled::PointerLock` asks for the pointer through `WindowRequests::pointer_locked`; whoever shows the document locks it, and motion arrives as `Event::PointerMotion` to the focused node only. A browser drops a lock on blur and Escape itself and refuses one without a user gesture; the document answers by clearing focus.
- **Pan and zoom.** `unstyled::PanZoom` turns gestures into a caller-owned `Binding<PanZoomView>`; momentum, snapping and clamping to content are the caller's (guides/pan_and_zoom.md).

## Drag and drop

Within a document: `Draggable` on what moves, `DropTarget` on where it lands, meeting on `unstyled::drag_board()`, one `Board` per document; nothing else wires them. Of the accepting targets under the pointer the one painted last wins, so nested slots each take their own drops; a target's `takes` says what it does with the item at a point (`Take`): `Drop` takes it, `Pass` lets the target beneath have the point, and `Refuse` keeps the target under the pointer, so it hears the item over it and can show it is refused, while a release there takes nothing and the source gets the item back (a Reorder glides it home). The board is written once, in unstyled `drag_board.rs`: each core only says where its nodes are (`beui_view::drag::space()`, a `Space`) and drives it (`drive`, a `Driver`): natively from the pointer each frame, across windows, and with Escape; on HTML from window listeners while a drag is carried, so it goes on after the element that began it is gone. Change drag behaviour there, and test it with its fake `Space` (`drag_board/tests`). The board also floats the item (a source's `floating`, unless the target under it `hides_preview`), tells every target the drag ended before the taker's `dropped`, and scrolls the innermost `Scroll` whose edge band (`EDGE_BAND`) holds the pointer. A finger drag on a `Draggable` in a scrolling list needs `touch_drags`, or the list scrolls. A drag between editors crosses apps and goes through the host (`Editor::drag`, guides/apps/host_services.md), which carries it on its own board with each app region a `DropTarget`.

### Reorderable lists

`unstyled::Reorder` (styled re-exports it, with a lifted look on `ReorderItem`) makes a container's items draggable into a new order: render `ReorderHandle::order`, not your keys, and make each item a `ReorderItem` that is the container's own child, since its root is a `Glide`. The list owns the board drag, so a `VirtualList` recycling rows is safe. A lifted item follows the pointer along the list and passes a neighbour once its leading edge crosses that neighbour's middle; pulled off the list it floats as the `preview`, and any `Reorder` that `accepts` the payload opens a gap. `on_move` hears one `ReorderMove { payload, key, after, index }` per drop or Alt+arrow (`step`); apply it synchronously, or the item glides back. `fixed` (with `slot`) is for an order decided elsewhere, a sort: items only come and go. `hover`/`drop_at` drive the list from a drag the board does not carry (a host's block drag), as `styled::Tree`'s `reorder` does.

## Touch and pen

A finger held still is a secondary press (tests set `Context::set_long_press_delay` to zero), and the press it began is cancelled (`on_cancel`). A finger drag across an `Interactive` scrolls whatever around it scrolls that way unless the catcher sets `touch_drags`. A finger landing on no control goes to the nearest within `TOUCH_REACH`; a catcher that only watches a region sets `claims_touch=false` so it does not take that reach. The pen behaves as the mouse.

## Time

beui has no per-frame hook. Anything time-based uses `create_timer(work)` (work returns the next delay or `None`) or `create_animation(step, finish)`, both owned by the creating scope, and nothing is drawn while none is due. A driven program settles only when no frame asks for another (guides/running/drive.md), so motion that never ends (an indeterminate bar, a blink) ticks from a timer with a frame-length delay rather than an animation, or `drive settle` waits forever. `tween`, `Spring` and `Fling` are the curves. `Glide` animates a move between layouts: bump its `arm` in the same update that reorders the keys (or hand it `from`, a window point to settle from), and a spring (`GlideMotion`, the one rule both cores step) takes its child from where it was to where it rests, laying out only the glide each frame. `hold_glide` pins a glide's child to a point, and `set_raised` paints a list child above its siblings. Under `Motion::Instant` animations complete at once and inertia is off, while functional timers still run.

## Preferences

`Preferences` (theme, motion, canvas touch; `crates/be-lib-ui-preferences`, shared with the app protocol) resolve from three layers, top down: `Developer` (the inspector), `Program`, `System` (the platform's, or the host's for an app). Components read `use_preferences()` and write a layer with `use_preference_layer(source)`; `RunOptions::saving_preferences(name)` saves a program's layer, and be-host keeps them in its display settings.

## Drawing with the gpu

A viewport no arrangement of nodes can make (a 3D scene, a map) is a `Drawing` painting `draw_gpu(drawing)`; its `Draw` runs in paint order with `prepare` (device, queue, encoder) and `paint` (beui's own render pass). `be_app_scene_3d` is the worked example, and `beui::native::texture_quad` shows a texture with no pipeline of your own. Traps:

- Key shared gpu resources by `DrawAt::renderer`: devices of two wgpu instances can compare equal.
- Leave the pass's viewport and scissor as found; beui keeps painting into it.
- Drawings compare by `Rc`, so a new one each frame keeps an idle editor repainting. Build resources on the first `prepare`, so headless tests never open a device.
- A crate under beui depends on beui-draw behind a feature beui turns on outside the HTML core (beui-components-styled's `gpu`).

## Writing an unstyled component

An unstyled component has no appearance defaults: colours, sizes, spacing, fonts and renderers for anything it must draw are required props the styled layer fills ([decision](../../decisions/beui_unstyled_layer_has_no_appearance_defaults.md)). Defaults that draw nothing and behaviour defaults (thresholds, zoom limits, delays) stay.

Put it in `crates/beui-components-unstyled/src/<name>.rs`, re-export it from `lib.rs`, and usually: user-changed state as a `Binding<T>` prop with no copy kept in sync (see `Toggle`); one focusable `Interactive`; the role and state through `component_accessibility`; a `Render<Handle>` or `RenderFn<Handle>` of reactive state for the face; and return the root base node directly.

- **Handles.** State callers or tests read is a `Copy` type (`SelectApi`) in an `api: ComponentRef<..>` prop filled with `api.fill(..)`; a test with only a `NodeId` reads `document.api::<SelectApi>(node)`.
- **Reaching the tree.** These crates do not name `Document`, `with_document` or `NodeOf` outside tests, so they build on any core; `be-tool-fix-rust-source` reports it. Use `beui::reactive` (`beui-view/src/nodes.rs`): `TreeNode` methods on `NodeId`, `measure`, `measure_text`, `focused_node`, `tree_local` and the rest. Add what is missing there.

### Pointer input

Where a catcher hears the pointer is declared ahead of time as `Area` memos (`hit_area`, `capture_presses`, `claims`; [decision](../../decisions/beui_hit_areas_are_declared.md)). A down is hit tested once (`interact::hit`); the press climbs from the topmost catcher to the first whose `capture_presses` holds it, arming each on the way, unless a modifier claim takes it first. A settled finger drag goes to a catcher holding that axis with `touch_drags`, else the first `on_pointer` answering `true`; the rest hear `on_cancel`. Input every catcher turns down is offered again with `unclaimed` set, which is how an outermost scroll still rubber-bands. Lists whose highlight follows the pointer use `pointed`, not `hovered`, so keys can walk under a resting pointer. A view that appears under a pointer that has not moved hears it as `PointerRested`: what is under it is `hovered`, but nothing is `pointed` until the pointer moves, whichever order the view and the rest arrive in (a menu opened under the pointer that pressed its button highlights nothing until the mouse moves). Only a host sends a rest, so a `BeuiTest` cannot reproduce one.

Gestures live in `unstyled::gestures` (`scroll_wheel`, `scroll_drag`, `autoscroll`, `secondary_drag`, `pan_zoom`, `first_of`), not in the core. A part drawn outside its catcher (a text area's touch handles) claims `Area::CaretHandles` and is painted from a later node, since paint order is tree order.

## Writing a styled component

Put it in `crates/beui-components-styled/src/<name>.rs` and re-export it. It wraps the unstyled control, passes `label` on, and paints the handle with base primitives; `checkbox.rs` is a short complete pair:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Binding<bool>) -> NodeId {
    view! {
        <Toggle checked label>|handle: ToggleHandle|
            <CheckboxFace handle />
        </Toggle>
    }
}
```

The face derives colours from memos over the handle's state and theme tokens, so it repaints when either changes. Write private consts as tokens. Add a field to `Theme`, with a value in every built-in theme, when a colour belongs to the theme rather than one component. Icons are `beui::icons` glyphs in a `styled::Icon` inside a `styled::Text`. Expected keyboard behaviour for each control family is in [../beui_keyboard.md](../beui_keyboard.md).
