# Styled component cleanup

An audit of `beui-components-styled` against `beui-components-unstyled` found
styled components that carry behaviour, components that don't belong in beui,
and parallel components that should be one. This plan is the agreed list of
changes. Implemented, except what each section records as left: the
inspector's compact App/Inspector bar (§4.3), true group opacity (§Polish:
`Fade` fades each shape on its own), and a rounded clip for plugin dialog
regions carried through the plugin protocol (§Polish).

## The rules this plan applies

- **Unstyled has no appearance defaults.** Someone using only the unstyled layer
  can implement any style without inheriting ours. Colours, sizes, paddings,
  breakpoints and placeholder renderers are required props at the unstyled
  layer and are chosen by the styled layer. Behaviour defaults stay allowed:
  the drag threshold, zoom limits and a slider's 0..1 range are what someone
  uses unstyled for instead of building their own. The tooltip delay is
  probably the same kind of thing. Leave it as a default unless doing §5
  turns up a reason not to.
- **Code outside beui depends only on the styled layer.** A crate that needs a
  face of its own is a sign of a missing styled component.
- **Behaviour lives in unstyled.** Focus, keys, pointer, touch, timers,
  animation driven by state, and accessibility nodes belong to an unstyled
  component. The styled component chooses fills, tokens and layout from its
  handle.
- **Product-specific components live in the product.** If one crate is the
  only possible user, the component lives in that crate.

## 1. Add the missing unstyled components

Each styled component below builds its own behaviour today. Move that behaviour
into a new unstyled component, then rebuild the styled one on top of it.

- **Done.** **`unstyled::Modal`**: an Overlay, a `BackSlide`, a scrim that fades with
  presence, and dismiss handling. `styled::Dialog` and `styled::Fullscreen`
  (`dialog.rs`, `fullscreen.rs`) copy this today. `ModalSheet` and the launcher
  overlay should share it where they can. `ModalSheet` is built on it
  (`ModalMotion::Reported`), and so is the command palette's overlay, which
  the launcher became in §3.
- **Done.** **`unstyled::Dialog`** on top of `Modal`: Enter confirms unless the action is
  destructive or disabled (`DialogActions::enter`), Dialog or AlertDialog
  accessibility with the title as its label, confirm and cancel callbacks.
  Make it placeable in a given rectangle and not only centred, so block-app
  can build keep-changes and the lock screen out of it (see §3).
  `DialogSurface` is `pub(crate)` today, so nothing outside styled can do
  that. It became `unstyled::Dialog` (the outer part, card or sheet) and
  `unstyled::DialogBody` (keys, accessibility, action order), with the
  styled `DialogFrame`, `DialogCard` and `DialogBody` placeable by anyone.
- **Done.** **`unstyled::Toasts`**: the stack, each toast's expiry timer, the rule that
  sticky and danger toasts don't expire, edge placement, the overlay that lets
  input pass through, and an accessibility live region. Toasts have no role
  today. Keep `Toast`'s notification fields (image, actions, activates,
  sticky). Toasts are generally useful with them. It takes the stack's
  `width`, `margin` and `spacing` and an `edge`, draws each card from a
  `ToastHandle`, and reads as a `Log` live region of `Status` toasts, with
  `danger` ones assertive `Alert`s.
- **Done.** **Indeterminate progress**: the phase animation, the reduced-motion check and
  the busy accessibility node from `styled::Spinner`. This becomes part of the
  merged `Progress` (§4.4). It is `unstyled::Progress`, whose handle carries
  the clamped `value` and the sweep's `phase`.
- **Done.** **`EveryScreen`**: one copy of something per screen, from `use_screens()`,
  each given its screen's rectangle relative to the bounds of all screens.
  `KeepChanges`, `LockScreen` and `LevelOsd` each repeat this. All three are
  moving out of beui (§3), but they still need it, in two different crates.
  All three used it until main's "a window per DRM display" (#696) removed
  screens from beui (`use_screens()` is gone; an overlay stays in its window).
  `EveryScreen` and `ModalPlacement::AllScreens` were then deleted: block-app
  shows its keep-changes prompt and lock screen once per display window, and
  linux-desktop's `LevelOsd` lays out one card per display rect from the host.
- **Done.** **Low priority:** an unstyled `Table` for the Table, Row, ColumnHeader,
  RowHeader and Cell accessibility nodes that `styled::Table` builds. An
  unstyled `ColorInput` for the preview state and the popover plus hex-field
  composition. Most of that is already in `HexText` and `Popover`.
  They are `unstyled::{Table, TableRow, TableColumnHeader, TableRowHeader,
  TableCell}` and `unstyled::ColorInput` (`swatch`, `picker` and `field`
  renders, laid out by its children).

## 2. Move behaviour out of styled components

- **Done.** **An unstyled action bar.** `DockMenu` (`beui-dock/src/styled.rs`, about lines
  690-850) decides which actions are pinned to the bar and which overflow into
  a "More" menu, by available room (`pinned`, `unpinned`, `BarRoom::shows`,
  `title_action`). Make that an unstyled action bar the dock uses. Any other
  toolbar can use it too. It is `unstyled::ActionBar` (with
  `unstyled::title_action`), drawn by `styled::ActionBar`, which the dock's
  bars and beui-demo's Menus page use.
- **Done.** **Tree row menus.** `TreeRowChrome` and `RowMenuButton` (`styled/tree.rs`
  240-357) wire each row's context menu. The More button finds its own
  rectangle with `placements().rect` and opens a context menu at that point.
  Make per-row menus a feature of `unstyled::Tree`, and make the More button a
  menu button anchored to itself. `TreeMenu` is unstyled's; the row handle
  carries a `TreeRowMenu` for `unstyled::TreeRowMenuButton`.
- **Done.** **Tooltips on icon-only buttons.** "Show the tooltip only when the button is
  icon-only and its popup is closed" is repeated in `MenuButton`, `Popover`,
  `ToggleButton` and `IconButton`. `ButtonHandle` already carries `tooltip`,
  so let the unstyled button decide when it shows. `unstyled::Button` and
  `unstyled::Toggle` take `icon_only` (and the button `expanded`), and the
  handle's `tooltip` is empty unless the button is icon-only and its popup is
  closed. The styled faces wrap themselves in a `Tooltip` of that text, since
  the bubble is a look; `SplitButton`'s arrow, the date picker's and the
  colour input's triggers lost their copies too.
- **Done.** **Dialog Enter handling**: moves with §1's `unstyled::Dialog`.
- **Done.** **Spinner animation**: moves with §1's indeterminate progress.
- **Done.** **TextInput clear and copy.** `styled::TextInput`'s clear and copy buttons
  (`copy_text`) are behaviour. Expose them from `unstyled::TextInput` as
  handle callbacks, and leave only the buttons' look in styled.

## 3. Move components out of styled

- **Done.** **`KeepChanges` → block-app.** It is a timed AlertDialog and only block-app's
  display settings use it. Once §1's Dialog exists, it should
  be a few lines in block-app. Delete `styled::KeepChanges` and its demo page;
  move its tests with it.
- **Done.** **`LockScreen` → block-app.** Only block-app uses it (`ui/lock.rs`), and
  nothing else ever would. Build it in block-app from styled pieces and §1's
  `Modal` (one per display window).
- **Done.** **`LevelOsd`.** Its bar becomes `Progress` (§4.4). What's left is an OSD card
  used only by linux-desktop (`app/media.rs`), so move it there. It is in
  `app/osd.rs` now, with its tests in linux-desktop's, and lays out one card
  per display rect.
- **Done.** **`Launcher`.** Its core joins the command palette (§4.2): a styled
  searchable palette with pluggable ranking and rows that show a picture or
  icon and a detail line. Also move `launcher_score` there as one of the
  ranking functions. Move everything else Launcher has into linux-desktop,
  next to `ProgramLauncher` (`editors/linux-desktop/src/app/launcher.rs`),
  the only product user: the program items, the "Run 'query'" row and the
  icons. be-launcher's `Launcher` is its own pull request list and doesn't
  use `styled::Launcher`. Move the launcher tests in `beui/src/tests`
  that test linux-desktop behaviour along with it; tests of the palette and
  ranking stay in beui.
  `plans/command_palette.md` also builds on `styled::CommandPalette`, so keep
  the two plans in step.
  `styled::CommandPalette` takes `items: PaletteItems` (`Actions`, the
  default, or `PaletteItems::list(..)` of `PaletteItem`s with a key, title,
  detail, terms, glyph, image, shortcut, checked and disabled), a `score`
  function (`PaletteItem::substring_score` by default, or
  `PaletteItem::fuzzy_score`, which is `launcher_score`), an optional
  `query` signal, and `on_choose` with the chosen key. linux-desktop builds
  its program items and the Run row (key `run`, scored below every program)
  itself. The beui run-as-command test was a copy of linux-desktop's
  `a_query_no_program_matches_runs_as_a_command`, so it went; the reopening
  tests and the filter test now drive the palette. The palette's rows are
  `RowFace`s (`RowSize::Large`), which gained a `shortcut` slot for them.
- **Done.** **`CrashNotice` and `StaleNotice` → block-app.** Only block-app uses them
  (`compositor/region.rs`, `surfaces.rs`). Their text is about plugins.
  They are in `surfaces/notices.rs`.
- **Done.** **`ThemeSelect` and `MotionSelect` → next to the settings editor** (the only
  user, `editors/settings/src/display/appearance.rs`), or into
  `block-editor-beui` beside `LayeredSettingsRow`. Delete
  `PreferencesSection`: only the demo uses it, and `LayeredSettingsRow`
  already does its job. They are private to the settings editor's
  `display/appearance.rs`.
- **Done.** **Dock → a new `beui-dock` crate** with an `unstyled` module and a `styled`
  module. Any beui user can then use the dock with their own styles. That is
  about 9,000 lines of unstyled (`dock.rs`, `dock/state.rs`, `docking.rs`,
  `spec.rs`, ...) and about 1,400 of styled. The unstyled dock depends only on
  beui-core, beui-view and unstyled pieces (menus, drag, choice, sheet, scroll),
  so the move should be mechanical. beui-inspector is the precedent for a beui
  crate outside the component crates.
- **Done.** **Software keyboard → a new `beui-keyboard` crate**, the same shape as
  `beui-dock`. beui itself depends on it (`beui/src/keyboard_host.rs`), like
  beui-inspector. The facade re-exports them as `beui::dock` and
  `beui::keyboard`, each with `unstyled` and `styled` modules
  (`beui::dock::styled::Docking`, `beui::dock::unstyled::DockState`,
  `beui::keyboard::styled::SoftwareKeyboard`). The dock's tests that drive
  it through the facade stay in `beui/src/tests`.
- **Done.** **Delete `Bordered`.** It is `Frame border=Border::DEFAULT radius=…`, and its
  one user is logic-grid.
- **Keep `styled::Stack`.** Its breakpoint is a style choice, so it belongs in
  styled (see the rules above).

## 4. Merge components

### 4.1 Every anchored popup is a dropdown on mouse and a sheet on touch

Only `MenuPopup` does this today. The "was it touched" tracking is copied in
`unstyled::MenuButton` and `unstyled::ContextMenu`. `Select` (which has its own
Overlay), `Popover`, the `DateTimeField` picker and `ColorInput`'s picker
always drop down. Add one unstyled anchored surface. Its style takes a
dropdown look and a sheet look, the way `MenuStyle` does, and it tracks touch
itself. Build `MenuPopup`, `Select`, `Popover`, the date and time picker and
the colour picker on it.

**Done.** `unstyled::Anchored` (`anchored.rs`) with `AnchoredStyle::new(dropdown,
sheet)`, `AnchoredSheetHandle`, `AnchoredHandle { sheet, shown }` and
`SheetWhen`. It samples `touched_by_finger()` (§1's `use_touch` narrowed to fingers,
so a pen's barrel button still drops a menu down) when it opens, so `MenuButton`,
`PopupTrigger` (its `on_press` is gone), `ContextMenu` and block-app's app menu
no longer track touch. `Popover` (and so `DateTimePicker` and `ColorInput`) and
`Select` take a required `surface: AnchoredStyle` in place of wrapping their
own panel; `styled::popover_surface(max_width)` is the popover's, and
`MenuStyle::with_sheet` now takes an `AnchoredSheetHandle` (`MenuSheetHandle`
is gone). A `MenuStyle` without `with_sheet` stays a dropdown
(`SheetWhen::Never`).

### 4.2 One searchable list

`unstyled::Select`, `unstyled::CommandPalette` and `styled::Launcher` each
implement a query, filtering or ranking, a highlighted row, arrow and Page
keys, scroll-to-row and Enter. Make one unstyled search list with a ranking
function. Use substring matching where those components use it today, and
`launcher_score`'s fuzzy ranking for the launcher. `Select`,
`CommandPalette` and the launcher's palette (§3) become stylings of it.
Check whether the text area's completion menu (`text_area/completion.rs`)
fits too.

**Done.** `unstyled::SearchList<K>` (`search_list.rs`) owns the query field,
the shown keys (from a `rank: Func<String, Vec<K>>` the caller passes),
the highlight, which skips disabled rows, Up/Down/Page Up/Page Down/Home/End,
scroll-to-row, Enter, clicks and hover. `rank`, `substring_score` and
`fuzzy_score` are the ranking helpers. `Select` and `CommandPalette` (and
through it linux-desktop's launcher) are built on it; their trigger,
popup, panel, search field and rows stay theirs. Home and End move the
highlight in every search field, as they did in Select's (not the caret). Filtered-out rows
are no longer built, so `SelectApi::option_button` returns an `Option`.

The completion menu doesn't fit, so it keeps its own list. Its query is a
span of the text area's own text, not a field of its own; its keys reach the
text area's key map while the caret stays in the document (and Tab
completes); its overlay passes input through instead of trapping the focus;
and the `Completer` returns its matches already ordered. Sharing would mean
splitting the search list's highlight and keys away from its field for one
user that has no scroll, no disabled rows and no Page keys.

### 4.3 Tabs

- **Done.** **Merge `ResponsiveTabs` into `Tabs`.** `Tabs` always collapses into a Select
  when it's narrow, the way a menu always adapts to touch. Today only the
  demo and its own test use `ResponsiveTabs`, and the real uses of `Tabs`
  never collapse. It collapses when its row of tabs doesn't fit the width it
  is given, not below a breakpoint: a new base node, `FirstFit`, lays out the
  first child whose natural width fits and leaves the rest unplaced (no paint,
  input or Tab stop). A fixed breakpoint would have collapsed the inspector's
  and the date picker's tabs, which sit in narrow panels with room to spare.
- **Done.** **Move the beui-inspector tool tabs to dock tabs.** Beui/Comp/A11y/Perf/Sim
  (`beui-inspector/src/panel.rs:259`) are independent panels, and side by side
  is useful. This needs `beui-dock` (§3). The tools are `DockTab`s of one
  pane in a styled `Docking` with `stack_below=0.0` (a new prop; it defaults
  to `NARROW_WIDTH`), so the narrow panel tiles rather than stacking. Each
  tree tool has its own rows, and the inspector collects rows only for the
  tree tools on show. Its layout isn't saved, like the rest of the
  inspector's state.
- **Later, maybe: the inspector's App/Inspector bar.** That compact-mode bar
  (`panel.rs:405`) is the dock's phone layout with the app as a pane. Revisit
  it once the inspector is a dock. Left as `Tabs`: the app isn't a beui node
  the inspector's document can hold, so it can't be a dock pane without the
  dock learning to reserve a pane for an outside surface.
- **Done.** **The block-app invite role is not tabs.** Editor/Administrator
  (`block-app/src/ui/dialogs.rs:596`) is a form value. It moves to the
  segmented control below.
- **Done.** **Keep these as `Tabs`:** the input-remaps Editor/TOML views
  (`editors/settings/src/input_remaps/ui.rs`) and the `DateTimeField` paged
  Date/Time picker.
- **Done.** **Add a styled segmented control** over `Choice`'s `Radio` kind, so it has
  radio semantics. Tabs switch a panel; a segmented control picks a value.
  Move calendar's Day/Week/Month, be-launcher's Open/Closed and block-app's
  invite role onto it. Today they use `Tabs` and report tab roles with no tab
  panel. It collapses into a Select when narrow, as `Tabs` does. Add it to
  beui-demo. It is `styled::SegmentedControl` (`options`, `selected: usize`,
  `label`); `Tabs` takes the same `label`, which names the tab list or radio
  group and the Select it becomes. `unstyled::Choice` gained `label` and
  `spacing`.

### 4.4 One Progress

**Done.** Merge `Spinner` into `Progress`: `value: Option<f32>`, where `None` is
indeterminate. Use §1's indeterminate progress for it. linux-desktop's OSD
card (§3) uses `Progress` for its bar, not a hand-drawn track and fill.
`Progress` takes `fill` and `track` colour roles, which the OSD uses to fade
and to grey the bar when muted, and a `width` that defaults to filling;
`INLINE_PROGRESS_WIDTH` stands in for the size token §7 adds.

### 4.5 One button, one menu button

- **Done.** `Button` takes `icon_only` and a size, and `IconButton` goes. `ButtonFace`
  already supports `icon_only`. `ButtonSize` is `Small` (the old icon
  `Compact`), `Compact`, `Regular` and `Toolbar`; `variant` defaults to
  `Secondary`.
- **Done.** `MenuButton`, `IconMenuButton` and `PictureMenuButton` become one menu button
  whose face is text, an icon or a picture. `IconMenuButton` has one product
  user (database). It takes `image`, `icon_only`, `size` and the picture
  button's click, secondary, middle-click and scroll props, and looks pressed
  while its menu is open.
- **Done.** `Popover`'s trigger reuses the menu button's trigger instead of copying it.
  Both are an `unstyled::PopupTrigger` (whose `PopupTriggerHandle` replaces
  `MenuButtonHandle` and `PopoverTriggerHandle`), and styled `Popover` paints
  the menu button's face.

### 4.6 One row button

`ListRow`, `ActionRow` (the menu sheet's row face as a standalone button),
`SettingsLink` and Launcher's private row each paint a pressable row with
hover and selection. Make one styled row with the slots they need (leading
picture or icon, label, detail, trailing chevron or check), and rebuild the
four on it.

**Done.** The row is `styled::RowFace` (`row.rs`) with a `RowSize` (`Compact`,
`Regular`, `Large`, `Touch`) and a `RowTrailing`, and `ListRow` also takes the
slots. The menu sheet's rows use it; the dropdown menu's rows keep their own
face, which has shortcuts and a different highlight.

### 4.7 Dialog, Fullscreen and KeepChanges share Modal

**Done.** These are covered by §1 and §3: `Dialog` and `Fullscreen` sit on
`unstyled::Modal`, and block-app's keep-changes prompt is a `Dialog` with a
countdown.

## 5. Remove appearance defaults from unstyled

**Done.** Following the first rule, these became required props and styled
passes them:

- **Done.** `TextInputStyle::default()` is gone (white text, a fixed
  placeholder, selection and caret colours): `TextInput`'s `style`,
  `SearchList`'s `style`, `Select`'s and `CommandPalette`'s `search_style`
  and `TreeRowEditor`'s `style` are required. `TextInputStyle::menu` is an
  `Option<MenuStyle>`: `None` says the field has no context menu.
- **Done.** `TextAreaColors::DEFAULT` and `SyntaxColors::DEFAULT` are gone.
  `TextArea`'s `colors` is required; `styled::text_area_colors(&Theme)` builds
  them from a theme, and the old dark syntax palette is the dark themes'
  `DARK_SYNTAX` in styled. A text input fills the area colours it has no use
  for (gutter, code, widgets) with transparent.
- **Done.** `SHEET_STOPS` as the default `stops` and `rest` of `Sheet` and
  `ModalSheet`: both are required at the unstyled layer, and `SHEET_STOPS`
  moved to styled (`styled::SHEET_STOPS`), whose `Sheet` and `ModalSheet`
  keep it as their default.
- **Done.** The text area's `BODY_SIZE` font size and `PADDING`: `font_size`
  and `padding` are required. `BODY_SIZE` stays inside the text area only as
  the reference size its heading and table ratios scale from.
- **Done.** `TimeList`'s 32px `row_height`.
- **Done.** The empty `<List/>` placeholders used when a trigger, row or
  option renderer isn't passed (`Select`, `CommandPalette`) were made
  required by §4.2.
- **Done.** `unstyled::Choice`'s `spacing`, which defaulted to 6px. The
  `Swatches` and the dock's tab strip, which are built on it, take a
  `spacing` and a `tab_spacing` of their own.

Found while doing it, and done the same way:

- **Done.** `MenuStyle::default()`, whose rows were empty placeholder lists,
  and the menu's hard-coded 2px row spacing. `MenuStyle` always has its row
  and panel renderers and is built with `MenuStyle::new(spacing, row,
  panel)`; `ContextMenu`, `MenuButton` and `SplitButton` require `menu`, and
  dock `Docking` requires `menu`. `Tree`'s `menu` and `menu_style` became one
  `menu: Option<TreeMenus<K>>`, made with `TreeMenus::new(style, menu)`, so a
  tree with row menus always has their style.
- **Done.** `CommandPalette`'s and `ModalSheet`'s `scrim` and dock
  `Docking`'s `dialog_scrim`, which defaulted to transparent, are required,
  like `Modal`'s.
- **Done.** `Selectable`'s blue selection `color` and `handle_color`.
- **Done.** Dock `Docking`'s `splitter_thickness` (`SPLITTER_THICKNESS`, 6px)
  and `icon_sidebar_width` (`ICON_SIDEBAR_WIDTH`, 40px), and the 3px width of
  the drop marker between tabs, now `drop_marker_width`; and the shade over
  the page behind the stacked dock's back gesture, now `back_shade`.
- **Done.** `BackSlide`'s hard-coded black shade over the page behind it is
  its `shade` prop, transparent unless given; be-launcher and the dock pass
  the theme's `scrim_soft`.
- **Done.** `Scroll`'s keyboard focus ring, a hard-coded 2px ring inset 1px
  in `focus_color`: it takes `focus_ring: Prop<Option<Outline>>` (none by
  default), and styled passes the accent ring.

Kept as defaults because they draw nothing: zero spacings, thumbs, radii,
insets and fades; `ScrollbarStyle::default()` (no bar); a picture's white
tint; pass-through panels; and the dock's optional pieces (frame, splitter,
grip, highlight, preview, ...) that draw nothing when not given. Kept as
behaviour: the drag and row-drag thresholds, zoom limits, slider range,
tooltip and toast durations, the dock's starting sidebar and floating window
sizes and its minimum pane sizes.

- **Done.** The text area's rich-document renderer drew with constants of
  its own: its default `leaf`, `marker` and `line` renderers (the markdown
  checkbox at 18px with a white tick, code and quote radii and bars) and its
  document geometry (`DOCUMENT_PADDING`, `LINE_PADDING`, run chips, inline
  widget pills and their white icon, the gutter, block spacing, table cell
  padding and delimiter size, `CARET_WIDTH`, the remote selection alpha).
  `TextArea` requires a `look: TextAreaLook` (the `leaf`, `marker` and
  `line` renderers and a `geometry: Prop<TextAreaGeometry>`); the default
  renderers are gone. `TextAreaLook::plain(geometry)` draws text and nothing
  else, and `TextInput` uses it with the new `TextInputStyle::geometry`, so a
  plain field never supplies a markdown renderer. Styled builds them with
  `styled::text_area_geometry()` and `styled::text_area_look()`, and
  `TextAreaColors` gained `remote_selection_alpha`. Kept in unstyled as
  proportions of the given `font_size`: the list, quote and margin `_EM`
  ratios and the heading size ratios. Block spacing and table padding in the
  geometry are given at `BODY_SIZE` and scale with `font_size`, so a zoomed
  canvas text keeps its proportions.

## 6. Get code outside beui onto the styled layer

**Done.** Outside beui's own crates, the only `unstyled::` import left is
infinite-canvas's text (`app/text.rs`, below), and nothing imports
`primitives::` (the source lint holds that).

About 29 crates import `unstyled::` directly. The biggest are block-shell (13
files), linux-desktop (12), settings (10), block-editor-beui (7),
infinite-canvas (6), database (6) and be-launcher (6). Audit each import: it
is either a styled component that doesn't exist yet, or a custom face that
should become a styled component. This is the largest item. Do it after §1-4,
which remove some of those imports on their own.

Outcomes, by crate group (each also migrated onto §7's tokens):

- **Done.** workspace-ui, linux-desktop, file-tree, settings, workspace-index,
  version-control, browser-tab, terminal and terminal-view import nothing from
  `unstyled` or `primitives`. Styled re-exports the behaviour-only pieces they
  needed (`BackSlide`, `ChoiceOption`, `Container`, `container_size`,
  `MenuItem`, `PopoverHandle`, `TextAreaState`, `TreeItem`, `TextChild`), and
  `dock::styled` re-exports the dock's model types (`DockNode`, `TabId`,
  `DockingLayout`, `DockApi`, `use_dock_tab`, ...). New in styled: `Picture`
  (radius as a `Radius`), `Span`'s `background: ColorRole`, `Button`'s
  `tab_stop`, `on_focus_change` and `revealed` (keeps its place and
  accessibility but paints nothing: file-tree's hover-revealed add button, which
  was a custom face), `TextColor::None`, and `Tree`'s `outline` returns a
  `ColorRole`. linux-desktop's level OSD fades a card drawn in colour roles
  with the base `Fade`'s `opacity` (see "Polish" below). Bar popups now share the
  popover width (`Size::Popover`) instead of each picking pixels.
- **Done.** database, logic-grid, deterministic-game, pixel-art, map,
  pixel-ray-tracer, calendar, counter, block-debug, beui-gallery, pan-zoom,
  checklist and be-launcher import nothing from `unstyled` or `primitives`.
  Database's own `ControlFace` is gone: its choice pickers are styled
  `MenuButton`s with a new `leading` slot (a node before the label in place of
  a glyph, here the choice mark), its filter chips and add-filter button are
  styled `Popover`s, and the filter's field choices are `ActionRow`s. The
  game's move-history cells are `ListRow`s, pan-zoom's rail button is a styled
  `Button`, and their dialogs use the default dialog width. Styled re-exports
  more behaviour-only pieces (`Draggable`, `DropTarget` and their handles,
  `Floating`, `Pressable`, `SliderScale`), and `dock::styled` the dock's
  document types (`DockDocument`, `DockDocumentBody`, `DockDocumentTabs`,
  `DockEntry`). Left on purpose: logic-grid's hotbar slot face and
  deterministic-game's board are plugin canvases with their own drawing;
  be-launcher's enlargeable picture is a `Pressable` around the content
  itself (labelled "Enlarge ..." for accessibility). The colour swatches in pixel-art,
  pixel-ray-tracer and map's presets became `SwatchButton`s (square ones take
  `radius=Radius::Xs`), added by the canvas group below. pan-zoom's cards scale their padding and outline with the
  zoom (`Space::Custom`, `Stroke::Custom`), and their radius too
  (`Radius::Custom`, added by the polish below).
- **Done.** **The canvas and media editors** (infinite-canvas, text-block,
  pdf, presentation, image-block, video-block, audio-block, scene-3d), with
  their §7 tokens. New styled component: `SwatchButton` (a round colour
  swatch that rings when selected, hovered or focused, with the colour's name
  as its label and tooltip, and a `radius`, round by default; the canvas's
  colour rows, and the misc group's palettes above). They use styled
  `Picture` too; `unstyled::Picture` now cuts a covering image's overflow
  from its source, so a radius rounds the frame's corners rather than the
  overflowing image's. Styled re-exports the behaviour-only pieces they
  used from unstyled: `Draggable`, `DropTarget`, `DragHandle`, `DropHandle`,
  `DragPoint`, `Floating`, `Edge`, `PointerLock`, `PointerLockHandle`,
  `ChoiceOption`, `MenuNodes` and the text area's data types
  (`TextAreaState`, `TextAreaColors`, `SyntaxColors`, `TextWidget`,
  `ProseLine`, `RemoteTextCursor`). One import stays on unstyled: the
  canvas's text (`infinite-canvas/src/app/text.rs`) is an `unstyled::TextArea`
  with `Prose`, because its font size, padding, colours and placeholder come
  from the canvas item and its zoom, and it has none of the styled text
  area's find bar or context menu. Colours left as `Custom` come from
  content: canvas, presentation and video colours, the PDF page, the black of
  a presented deck and its playback shade, and the video stage. The shared
  embedded-editor geometry (`block_ui::EMBEDDED_EDITOR_*`), the timeline's
  lane geometry and the hex view's measured size are `Space::Custom` and
  `Size::Custom`, since other code computes with the same pixels.
- **Done.** block-app, block-shell, block-editor-beui, block-ui-test,
  paint-review, be-wayland and beui-platform-plugin's tests (paint-review-web and
  beui-web-demo had nothing to move). None imports `unstyled::` any more, and
  they build without the §7 conversions.
  - New styled components: `TileButton` (a square pressable tile around any
    picture, `selected`, `outlined`, `radio` for radio semantics, `description`,
    `size`), used by block-shell's rename icon grid and block-editor-beui's
    rotation picker; `SwatchButton` (the same one the canvas group added), used
    by the rename colours. `ListRow` gained `glyph_color` and `size`, so the
    picker's More stage rows are `ListRow`s rather than a face of their own.
  - Re-exported from styled, since they carry no look: `Container`,
    `container_size`, `narrower_than`, `Edge`, `Floating`, `MenuItem`,
    `MenuCaption`, `MenuGrid`, `MenuThen`, `TreeItem`, `ChoiceOption`,
    `DragPoint`, `DropHandle`, `DropTarget`, `PanZoom`, `PanZoomHandle`,
    `PanZoomView`, `RemoteTextCursor`, `TextAreaState`; and from
    `dock::styled`, `DockDialog`, `DockDialogPresentation` and `DockEntry`
    beside the dock model types.
  - block-editor-beui's `Sidebar` takes `width: Size` (`SIDEBAR_WIDTH` is
    `Size::Custom(260)`) and `Toolbar` takes `spacing: Space`.
- **Done.** Leftovers. block-editor-beui's `BottomDock` and infinite-canvas's
  `ZoomPill` drew the same floating pill, so it is one styled component,
  `ToolPill` (a raised, bordered, fully rounded row of tools with arrow-key
  focus that scrolls sideways when squeezed; beui-demo Buttons page "Tool
  pill"). The dock's corners went from `Lg` to `Full` to match the pill. The
  plugin protocol's `settings-progress` (`changed`, `summary`) is gone: since
  artifact settings became a plugin dialog nothing read it, and the host's
  `ArtifactSession::summary` went with it. beui-demo's styled pages use the
  styled re-exports (`MenuThen`, `MenuItem`, `MenuNodes`, `MenuCaption`,
  `MenuGrid`) rather than `unstyled::`; their two custom button faces on
  `unstyled::Button` stay, as samples of building a face on the unstyled
  layer. Dock-like bars under comfortable density: block-editor-beui's
  `Toolbar` scrolls when chrome is narrow and text-block's format bar
  scrolls, so neither can overflow a phone; linux-desktop's bar and
  be-launcher never resolve comfortable (only `BeuiFrame` on a phone
  provides it, and the desktop is a root editor without a phone top bar).

## 7. Styled components take design tokens, not raw values

**Done.**

Text already takes tokens: `TextSize`, `Weight`, `Family`, `TextColor`. The
rest of the styled layer still takes raw values. Outside beui, about 205 files
use styled. Between them they have about 740 `spacing=`, 180 `padding=`, 33
`Radius::custom`, 56 `ColorRole::Custom` and 17 `TextColor::Custom`, plus
about 190 private `*_SPACING`, `*_PADDING`, `*_INSET` and `*_MARGIN` consts.
Styled has about 190 of those consts of its own.

- **Done.** **Spacing tokens.** Add a `Space` scale, named like `Radius` and `TextSize`.
  It resolves through `Density`, so compact and comfortable layouts come from
  the tokens instead of from each component (`Tree` has its own `Metrics` for
  this today). The values in use are 0, 2, 4, 6, 8, 10 and 12, then a long
  tail of 14, 16, 18, 20 and 24. Use the steps 0, 2, 4, 6, 8, 12, 16, 20 and
  24. Snap 10, 14 and 18 to whichever neighbour looks right in each place,
  and accept the paintings that change.
- **Done.** **Styled `Frame`.**
  - `padding` takes `Space`, on all sides or on each side.
  - `border` width and offset take tokens: hairline, focus ring and focus
    ring offset. `FocusRing`'s `offset` uses the same token.
  - `Radius::Custom` goes. Add the radii it's covering for: the 7px toolbar
    and tab radius, and the "Md plus 2" focus-ring radii, which become one
    derived token.
  - `width` and `height` keep `Extent` for fill, fraction, intrinsic and
    at-most. Fixed pixel lengths (about 30 raw, more through consts) split
    two ways. Control and picture sizes become size tokens
    (`TextSize::icon_px` already exists for icons). Lengths that are content
    (a thumbnail, a board cell, a canvas) use a `Custom(f32)` escape hatch
    on the size token.
- **Done.** **Styled `List` and `Grid`.** Styled re-exports the base `List` and `Grid`
  unchanged, so `spacing` is raw. This is the biggest case, with about 740
  uses. Add styled versions whose spacing takes `Space`, and stop
  re-exporting the base ones. Do the same for `Gap` and `Spacer` widths in
  styled `Text`.
- **Done.** **Complete `ColorRole`.** `ColorRole::Custom(theme.dim.get())` exists
  because the theme has colours that have no role: `danger_fill` and its
  hover and active, `danger_soft`, `on_danger`, `dim`, `scrim`, `scrim_soft`,
  `scrim_strong`, `control_outline` and `knob_outline`. Add them, so code
  never reads a theme colour to hand back as a custom one. Do the same for
  `TextColor`.
- **Done.** **Colours from content.** After that, `ColorRole::Custom` and
  `TextColor::Custom` stay as escape hatches. They are for colours that come
  from content: a user's tag colour, a remote cursor, a presentation
  background, a chart series, a colour swatch. Don't use them for a colour
  the theme has or should have.
- **Done.** **Component props in pixels** move to tokens:
  - Size tokens or a named size: `Dialog`'s `width`, `Popover`'s
    `max_width`, `MenuPopup`'s `min_width`, `Calendar`'s `width`,
    `Select`'s `width`, `Spinner`'s `width` (merged into `Progress`) and
    `Separator`'s `length`.
  - `Space`: `Tree`'s `spacing` and `padding`, and `styled::Stack`'s
    `spacing`.
- **Done.** **Inside styled too.** Rewrite the private consts in styled (button and
  row paddings, gaps, focus offsets) in tokens. Tokens that styled itself
  doesn't use aren't real.
- **Done.** **The types enforce it.** Once styled props take token types, a raw `8.0`
  doesn't compile. One lint is still needed: a styled `Length` takes an
  `Extent`, and `Extent::px` writes pixels into it, so fix-rust-source rejects
  `Extent::px` outside beui's own crates (the same files the primitives rule
  covers; beui's crates hand `Extent::px` to base frames legitimately). Unstyled and base components keep
  taking concrete values, as the design principles say.

### What landed (the token foundation)

All in `crates/beui-components-styled/src/tokens.rs` unless noted, exported
from `beui::styled`:

- `Space`: `None` 0, `Xs2` 2, `Xs` 4, `Sm` 6, `Md` 8, `Lg` 12, `Xl` 16, `Xl2`
  20, `Xl3` 24, plus `Custom(f32)` for lengths that come from content (a
  line's indent, a zoom-scaled inset). `Space::resolve(Density)`: under
  `Density::Comfortable` every step from `Xs` up moves one step larger (4→6,
  6→8, 8→12, 12→16, 16→20, 20→24, 24→32); `None`, `Xs2` and `Custom` never
  scale. `Space::px()` is the compact value. `use_space(space) -> Prop<f32>`
  resolves through the nearest density for an unstyled prop that takes pixels.
- `Padding { left, top, right, bottom: Space }` with `all`, `symmetric`,
  `horizontal`, `vertical`, `new`, `.left(..)` and siblings, `ZERO`, and
  `Padding::custom(Sides)` for content insets. A `Space` converts into an
  all-sides `Padding`.
- `Size`: `Track` 6, `Xs` 16, `Sm` 20, `Md` 24, `Lg` 32, `Xl` 40, `Xl2` 48,
  `Xl3` 64, `Row` (22, or 44 when comfortable: the tree row, folded from
  Tree's old `Metrics`), `Toolbar` 30, `Field` 34, `TouchRow` 52, `Menu` 200,
  `Popover` 320, `Dialog` 360, and `Custom(f32)` for content lengths (a
  thumbnail, a board cell, a canvas, a measured width). `use_size`.
- `Length`: `Length::Extent(Extent)` for intrinsic, fill (`Length::FILL`),
  fraction and at-most/at-least, or `Length::Size(Size)` for a fixed length.
  Styled `Frame`'s `width`/`height`, `Progress`'s `width`, `Calendar`'s
  `width`, `Select`'s `width` and `Separator`'s `length` take it; an `Extent`
  or a `Size` converts into it.
- `Stroke`: `None` 0, `Hairline` 1, `Focus` 2, `Custom(f32)`. `Border`'s
  `width` and `offset` are `Stroke`s (`Border::new` is hairline, `Border::FOCUS`
  is a focus-width accent ring). `FocusRing` takes `radius` (the radius of what
  it rings) and `offset: Stroke`, and draws the ring at `radius + offset`: that
  is the derived focus-ring radius ("Md plus 2" is `radius=Radius::Md
  offset=Stroke::Focus`).
- `Radius` gained `Tab` (7: toolbar buttons, compact buttons, dock tabs) and
  lost its old `Custom` (a later polish brought back `Radius::Custom(f32)`
  for corners from content only). A frame rounds only some corners with `corners: CornerSet`
  (`ALL`, `TOP`, `BOTTOM`, `LEFT`, `RIGHT`, `CornerSet::new(..)`).
- `ColorRole` gained `DangerFill`, `DangerFillHover`, `DangerFillActive`,
  `DangerSoft`, `OnDanger`, `ControlOutline`, `KnobOutline` (transparent when
  the theme has none), `Dim`, `Scrim`, `ScrimSoft`, `ScrimStrong`. `TextColor`
  gained `AccentHover` and `AccentActive`. `ButtonVariant::fill` and
  `theme::field_border` return a `ColorRole`.
- Styled `List(direction, align, justify, wrap, spacing: Space)`, `Grid(columns,
  column_spacing: Space, row_spacing: Space)` (`list.rs`) and `Gap(width: Space)`
  (`text.rs`). Styled no longer re-exports the base `List`, `Grid` or `Gap`.
- Props: `Dialog`/`DialogFrame`/`DialogCard` `width: Size` (`DIALOG_WIDTH` is
  `Size::Dialog`), `Popover`/`PopoverPanel` `max_width: Size`, `MenuPopup`
  `min_width: Size`, `Tree` `spacing`/`padding: Space`, `Stack` `spacing:
  Space`, `INLINE_PROGRESS_WIDTH` is `Size::Xl`. Removed from `styled::theme`:
  `BORDER_WIDTH`, `FOCUS_RING_WIDTH`, `SEPARATOR_THICKNESS`,
  `SCROLLBAR_TRACK_WIDTH`, `SCROLLBAR_INSET` (use `Stroke`/`Size`/`Space`, or a
  `Separator`).
- Styled, beui-dock's and beui-keyboard's styled modules, beui-demo and
  beui-inspector are written in tokens and build without the temporary
  conversions below.

Left deliberately: dimensions particular to one component (a calendar day
cell, the palette's 560 width, the colour picker's area) stay private
`Size::Custom` consts or `Extent::at_most(..)` inside styled rather than
growing the scale with tokens nothing else uses. Unstyled and base components
keep taking pixels; styled resolves tokens before handing them down.

### The temporary conversion, and migrating a crate

**Done.** The conversions below are deleted, along with the `Px` variants,
`Radius::custom`, `Padding::from_sides`, and `Border::width`/`offset`'s
`impl Into<Stroke>` (they are `const fn`s taking a `Stroke`). The value-to-token
table in steps 1 to 5 still holds for new code.

So that crates outside beui kept compiling while they were migrated one at a
time, these conversions existed and were temporary. Each maps a raw value to a
`Px` variant (`Space::Px`, `Size::Px`, `Stroke::Px`) that
behaves like a fixed length:

- `f32` (and `Prop`/`Memo`/`Signal` of it) into `Space`, `Padding`, `Size`,
  `Length` and `Stroke`; `Option<f32>` into `Length`; `Sides` into `Padding`;
  `f32` into `Stroke` for `Border::width`/`offset` (`impl From<f32> for
  Stroke`); `Radius::custom(f32)` (now a `Radius::Custom`, which stays:
  see the polish below).

To migrate a crate (one PR per crate group):

1. Replace each raw value with the token for it. Spacing and padding (`spacing=`,
   `padding=`, `Sides::..`, `*_SPACING`/`*_PADDING`/`*_INSET`/`*_MARGIN`
   consts): 0→`None`, 2→`Xs2`, 4→`Xs`, 6→`Sm`, 8→`Md`, 12→`Lg`, 16→`Xl`,
   20→`Xl2`, 24→`Xl3`. Snap the rest to whichever neighbour looks right there:
   1 and 3 → `Xs2` (or `Xs`), 5 → `Xs`, 7 and 9 → `Sm`/`Md`, 10 → `Md` (or
   `Lg` for page-level gaps), 14 → `Lg` (or `Xl`), 18 → `Xl` (or `Xl2`), 28 and
   32 → `Xl3`. `Sides::symmetric(a, b)` becomes `Padding::symmetric(Space::A,
   Space::B)`; a padding built from measured or scaled values becomes
   `Padding::custom(sides)` or `Space::Custom(x)`.
2. Fixed widths and heights: controls and pictures take the `Size` scale
   (16, 20, 24, 32, 40, 48, 64, `Toolbar` 30, `Field` 34, `TouchRow` 52,
   `Track` 6); panels take `Menu`/`Popover`/`Dialog`; a length that is content
   (a board cell, a thumbnail, a canvas, a computed width) is `Size::Custom(x)`.
   Fill, fraction and at-most stay `Extent` (`width={Extent::fraction(1.0)}`).
   Don't write `Extent::px(..)` on a styled frame; use `Size`.
3. Radii: `Radius::custom(3.0)` → `Xs`, 4/5 → `Sm`, 6 → `Md`, 7/8 → `Tab`,
   10-12 → `Lg`, half the side of a round thing → `Full`. One-sided rounding:
   `radius=Radius::Lg corners=CornerSet::TOP`. A focus ring around a rounded
   thing: `<FocusRing radius=<that radius> offset=Stroke::Focus>`.
4. Borders: `.width(1.0)` → `Stroke::Hairline`, `.width(2.0)` →
   `Stroke::Focus`, `.offset(2.0)` → `Stroke::Focus`; a zoom-scaled outline is
   `Stroke::Custom(x)`. A 1px rule `<Frame width=BORDER_WIDTH fill=Border/>`
   is a `<Separator direction=Direction::Vertical />`.
5. Colours: replace `ColorRole::Custom(theme.x.get())` / `TextColor::Custom(theme.x.get())`
   with the role (`ColorRole::Scrim`, `ColorRole::DangerFill`, ...); a memo
   that picks a fill by state returns a `ColorRole` and is passed as
   `fill={memo}`. `Custom` stays only for colours from content.
6. To find every site, build the crate with the conversions switched off:
   temporarily delete the `into_prop_via!` lines after `into_prop_via!(Length,
   Extent, ..)` in tokens.rs and `impl From<f32> for Stroke`, make
   `Border::width`/`offset` take `Stroke`, and rename `Radius::custom`; then
   `./scripts/buck build //crates/<crate>:<crate>` lists them. Put the files
   back before committing.

Final step, once every crate is migrated: delete those conversions, the `Px`
variants of `Space`/`Size`/`Stroke`, `Radius::custom` (its callers write
a token, or `Radius::Custom` for a corner from content),
`Padding::from_sides`, and make `Border::width`/`offset` take `Stroke`
again (they take `impl Into<Stroke>` only for the conversion). Then
consider a source lint against `Extent::px` on a styled `Frame`. **Done**:
the last callers were beui's own tests and examples, beui-demo's tests, and
pixel-art's, pixel-ray-tracer's and logic-grid's sidebar widths
(`Size::Custom`); the lint is above, under "The types enforce it".

### Polish after the migrations

- **Done.** A selected `RowFace` (so `ListRow`) under the pointer kept the
  hover fill and looked unselected. It now fills with a new
  `ColorRole::AccentSoftHover` (a `Theme` field in every built-in theme)
  while selected and hovered; `Tree`'s rows do the same. Painting
  `beui-demo.a_selected_row_stays_selected_under_the_pointer`.
- **Done.** `ButtonFace`, `MenuButton` (and the popover's face) take
  `placeholder: Prop<bool>`, which mutes the label (`TextColor::Muted`) while
  it stands in for an unset value. Database's choice pickers ("Set status")
  pass it while nothing is chosen; beui-demo's Menus page "Menu buttons" sample
  has a "Set priority" button that shows it.
- **Done.** New `styled::Banner` (`banner.rs`), on a new `unstyled::Banner`
  (alert role, assertive live region labelled with the message): an inline
  danger bar with the message and a Dismiss button, shown while its
  `message: Prop<Option<String>>` is `Some`; test ids `{id}` and
  `{id}.dismiss`. text-block's and infinite-canvas's import-error bars and
  database's error row use it. beui-demo Overlays page "Banner" sample,
  painting `beui-demo.a_banner_says_what_failed_until_it_is_dismissed`. Left:
  map's sidebar error is a caption over a button in a narrow column, which
  reads better than a bar there.
- **Done.** scene-3d's walking clock and video-block's playback clock read
  `now()` (the frame clock) instead of `Instant::now()`.
- **Done.** be-launcher's enlargeable picture is labelled "Enlarge
  {alt text}". No test: building `WebImage` needs a launcher `Model`.
- **Done.** calendar's event dialog keeps the default `Size::Dialog` (360):
  painting `calendar.an_event_opens_in_a_form_wide_enough_for_its_fields_and_actions`
  shows the title field, both date-time fields and Delete, Cancel and Save
  fitting with room to spare, so the old 420 bought nothing.
- **Done.** `Radius::Custom(f32)` replaces the temporary `Radius::Px(Corners)`
  and stays, like `Space::Custom` and `Size::Custom`, for a corner that comes
  from content: pan-zoom's cards scale it with the zoom, as they already
  scale their padding, outline and text. A fixed `Radius::Lg` on a card
  zoomed far out rounds it into a pill, and zoomed in looks sharp beside its
  scaled outline. `Radius::custom(f32)` is now a temporary alias for it
  (block-editor-beui's two callers), deleted in the final step.

## 8. One `overflow` prop for text, defaulting to wrap

**Done.** `Overflow` and `WrapAt` live in `beui_core::font` (re-exported as
`beui::Overflow` and `beui::WrapAt`); `Wrap`, `clip` and `ellipsis` are gone
from the base node, `primitives::Text` and `styled::Text`. Measuring added
`Element::min_width` (memoised like baselines), which `Text`, `Frame`,
`List` and the one-child wrappers answer; a horizontal list floors its
`Percent` and shrinking children at it. The terminal's screen rows stay
`Visible`: `Ellipsis` cuts each `Break`-separated row on its own, but would
draw an ellipsis the terminal's cells don't hold while the view is narrower
than the grid. The paintings are beui-demo's "Wrapping and ellipsis" sample
on the Text nodes page.

beui text overflows its container by default. `primitives::Text` and
`styled::Text` default to `wrap: Wrap::Off`, `clip: false` and
`ellipsis: false`. Text is measured at its natural width and painted
unclipped (`beui-core/src/base/text.rs`: `wrap_width` around line 420,
`measure` and `paint` further down). Ellipsis is ignored when wrapping (the
`self.ellipsis && self.wrap == Wrap::Off` check around line 560, and
`RichLayout::truncate` in `rich.rs`). There is no option for a maximum
number of lines.

Replace `wrap`, `clip` and `ellipsis` in the base node, `primitives::Text`
and `styled::Text` with one prop:

```rust
overflow: Overflow {
    Wrap { at: WrapAt, max_lines: Option<u32> }, // WrapAt: Words | Anywhere
    Ellipsis,
    Visible,
}
```

It defaults to `Wrap { at: Words, max_lines: None }`.

- **`Wrap`** breaks lines to the width. With `max_lines`, the last line ends
  in an ellipsis. `Anywhere` stays for monospace text laid out to a column
  count (terminal lines, `beui-demo/src/base_pages.rs`).
- **`Ellipsis`** is for one-line labels.
  - A hard line break in the content (a newline or a `Break`) still starts a
    new line, and each line is cut on its own.
  - Check whether that's right for the terminal before relying on it.
- **`Visible`** is today's `Wrap::Off`: natural width, unclipped. It is for
  text inside something that scrolls or clips for it:
  - the text area with wrapping off (`text_area/lines.rs:581`)
  - terminal rows drawn with `Break`s (`terminal-view/src/lines.rs:158`,
    where a soft-wrapped row must copy as one line)
  - map labels (`editors/map/src/app/labels.rs:217`)
- **Measuring.** Wrapped text reports a minimum width (its longest run that
  can't be broken) and a natural width, so it works in horizontal lists and
  intrinsic sizing. A full layout stays O(n).

Convert every use:

- `ellipsis=true` (about 41) becomes `Ellipsis`. The places include:
  - `Button`'s label
  - `select.rs`, `action_row.rs`, `context_menu.rs`
  - `launcher.rs`, `beui-dock/src/styled/stack.rs`,
    `beui-dock/src/styled/switch.rs`
  - `beui-keyboard/src/styled.rs`
- `clip=true` becomes `Ellipsis`: `beui-dock/src/styled.rs` (twice), `number_input.rs` and
  `editors/text-block/src/app/large_embed.rs`. There is no `Clip` mode. All
  four are titles or a value where text cut off mid-letter reads as the
  whole thing; an ellipsis says something is missing. Text that genuinely
  needs clipping is `Visible` inside something that clips, such as a
  scroll.
- `wrap=Wrap::Words` (about 222) is the new default, so delete it.
- `wrap=Wrap::Off` becomes `Visible`, and `Wrap::Anywhere` becomes
  `Wrap { at: Anywhere, .. }`.
- Callers that rely on the old default and should be one-line labels get
  `Ellipsis`: `chip.rs`, `choice.rs`, `select.rs` around line 170,
  `checkbox.rs`, and row labels in editors and block-app. Read each one;
  don't convert them blindly.

Add paintings for wrap, for wrap with a maximum number of lines, and for
ellipsis. Update guides/beui.md, which says to give paragraphs
`wrap=Wrap::Words` (around line 3693) and describes the base `Text` props
(around line 640).

## 9. One way to lay out a modal's buttons

Today a modal's buttons end up in one of four places, and which one depends
on who wrote it:

- **`Dialog`'s footer.** `confirm`, `cancel`, `destructive` and
  `extra_actions`. About 16 dialogs use it. Extra actions mix Delete
  (danger, calendar), Copy (secondary, export) and others.
- **Inside the content, where the author put them.**
  - Invite: "Create invite link", with a `Spinner` placed by hand.
  - Members: an inline Remove/Keep. Remove is a blue Primary although it's
    destructive, and the dialog has no visible close.
  - Share: Add and Invite in the content, Done in the footer.
  - Text settings, About and logic-grid bits have no footer at all.
  - pixel-art imports `Dialog` as `Modal` and draws its own buttons.
- **Sheets, with no convention.** database "Filter and display" puts a ghost
  Done top right, and phone details has none. block-shell's picker switches
  between `Dialog` and `ModalSheet` by width itself (`picker_view.rs:477`).
- **Dock dialogs.**
  - A window has a title-bar X.
  - Rename, a card, draws its own X and has no confirm button.
  - Plugin editors opened as dialogs draw whatever they like.

### Actions are data with roles; the modal places them

- **Done.** **Actions are data.** An action has a label, a role (`Primary`, `Cancel`,
  `Destructive`, `Secondary`), `disabled`, `busy` and `on_click`. The modal
  decides placement, order, style, focus and keys. This replaces `confirm`,
  `cancel`, `destructive` and `extra_actions`. It lives in §1's
  `unstyled::Dialog`, and styled paints it.
- **Done.** **Two kinds**, as GNOME's libadwaita has them.
  - **An alert** is a decision: leave, discard, delete, approve. Its actions
    go in the footer and its role is AlertDialog. Enter does the primary
    unless it's destructive. A destructive primary is red, and focus starts
    on Cancel.
  - **A dialog** is content: settings, members, inspect, about. It has a
    title row with a close X, and a footer only when there's something to
    confirm. Every modal has a visible way out.
- **Done.** **Wide layout.** The footer is right-aligned: secondary actions on the left,
  then Cancel, then Primary (the macOS and GNOME order). It stays put while
  the content scrolls.
- **Done.** **Narrow or touch layout.** A dialog becomes a sheet on its own, the way
  menus do, so the picker's hand-written switch goes. An alert stays a
  centred card. In both, the footer is pinned to the bottom with full-width
  buttons, primary first, and stays above the on-screen keyboard. Narrow is
  the widest screen under `SHEET_BELOW` (400, so a narrow desktop window;
  phones are caught by touch), touch is the press that opened it. beui's own
  keyboard shortens the screen, so the sheet sits above it; a platform
  keyboard does the same only where the platform resizes the window.
- **Done.** **Busy actions.** `busy` disables the action and shows progress in its
  button. This replaces the hand-placed spinners. `Button` has `busy` for
  the same thing outside a footer (share's Invite).
- **Done.** **Confirmations inside a dialog are nested alerts.** Members' Remove/Keep
  becomes an alert with a red Remove.
- **Done.** **Row and control actions stay in the content**: a member's role, removing
  a member, share's Add. Only the modal's own actions go in the footer.
- **Done.** **Convert every modal listed above**, the dock's Rename included.
  The block picker's More and Link stages are `DialogBody`s too (in its
  centred card and its dock card alike), and its creation stage is the
  plugin's own `DialogBody` (below). Left as they are: phone details, which is
  a menu sheet of rows rather than a dialog; members' busy spinner, which
  shows row actions in flight.

### The dialog's inside is one component that plugins use too

Split the dialog into an outer part and an inner part:

- **The outer part** belongs to whoever shows the dialog: placement, scrim,
  presence, the card or sheet frame, back and dismiss.
- **Done.** **The inner part** is a styled `DialogBody`: the title row with its close
  X, the scrolling content, and the pinned footer with role actions, laid out
  wide or narrow. `Dialog` and `ModalSheet` use it. (`DialogFrame` is the
  outer part: a card, or a `ModalSheet` holding the body.)

**Done.** A plugin opened as a dialog gets the whole area inside the frame, with no
padding and no shell title bar. A beui plugin builds its contents with the
same `DialogBody`, so it defines its own buttons with the code beui uses for
its own dialogs. The plugin protocol stays free of beui: it carries only
what any framework needs.

The plugins opened as dialogs today are the creation dialogs of templates
marked `"dialog": true` (image, PDF, audio, game, game module), which the
picker shows as a `Subregion dialog=true`; their `Creation` components are
`DialogBody`s (`ContentFileCreation` for the file pickers) whose Create calls
`creation.accept()` and whose Cancel calls `creation.cancel()`. The protocol
gained `frame-spec.dialog` and `child-placement.dialog` (a `dialog-frame`:
`narrow`, `bottom-inset`), `shell.handles-dismiss`, `shell.close-dialog`
(`dialog-answer`: `dismissed` or `accepted`), `shell.dismiss-child`, the
plugin's `editor.dismiss`, and `child-status.dialog` (`dialog-status`);
`creation-ready` went, since the plugin now enables its own Create, and
`creation-progress.options` lost its `ready`. block-editor-beui hands the
plugin's `DialogBody` a `DialogLayout` and an `unstyled::DialogDismissal`
(`crates/block-editor-beui/src/dialog.rs`). **Done:** the artifact settings
dialog is the same: block-shell shows a `DialogFrame` around a `Subregion
dialog=true` of `ArtifactSettings`, and block-editor-beui puts the editor's
`ArtifactSettings` component inside a `DialogBody` ("Dynamic artifact
settings", Cancel and Apply) whose Apply is enabled once the copy differs from
the stored settings (a plugin's `update` now gets a `SettingsDraft` with both)
and answers `close-dialog(accepted)`; the shell then commits the child as
before. The host reports the artifact instance's `dialog-status` on the
settings child, routes a `dismiss-child` of it to that instance, and forgets
its answer once the settings close.

- **Done.** **Back and dismiss go to the plugin first.** Back already reaches a plugin
  as input. Escape, back and a press on the scrim reach the plugin, whose
  `DialogBody` runs its Cancel action. That way a form with unsaved changes
  can ask before closing. The plugin asks the host to close it. The host
  closes it itself only if the plugin doesn't answer. "Doesn't answer" is
  decided up front, not by waiting: the plugin says `handles-dismiss(true)`
  while a `DialogBody` is mounted (it claims the `DialogDismissal`), and until
  then, or once the instance stopped, the host answers `dismiss-child` for it
  with `dismissed` at once.
- **Size.** The plugin reports the height it wants, so a card or sheet fits
  its content, and the host passes back the size it got. Reuse the
  intrinsic sizes the protocol already has for child placements if they
  fit. **Done:** a creation instance already reported the height its root
  measures as its frame's content, which the host passes on as the child's
  intrinsic height; the picker sizes the card to it, and the size the plugin
  got is its frame's.
- **Done.** **Keyboard and safe area.** The host tells the plugin the bottom inset (the
  on-screen keyboard, a phone's gesture bar), so the pinned footer stays
  visible. `dialog-frame.bottom-inset` is how far the child reaches below the
  document's viewport (beui's keyboard and the platform's safe area already
  shorten it) or the parent's own `DialogLayout::bottom_inset`, whichever is
  more; `DialogBody` adds it to its bottom padding.
- **Window dialogs** keep a frame but no title bar. Moving one by dragging
  its title row needs the plugin to report that row as a drag region. Check
  whether anything moves dialogs today before adding that. **Checked:** no
  plugin editor is shown in a window-presentation `DockDialog` today (only
  program windows are), and dock dialogs are anchored, not dragged, so no
  drag region was added. The dock's card dialogs now provide a
  `DialogLayout`, narrow while the dock is stacked.
- **Other frameworks.** A plugin written with another framework (egui, say)
  gets the same area and messages and draws its own version. Only beui
  plugins match the shell's look exactly, which follows from keeping the
  protocol free of beui.

## Polish

Problems earlier steps found in the beui crates, fixed at their source:

- **Done.** **A modal asks before it slides away.** `unstyled::Modal`'s
  default `Slides` motion ran its `BackSlide`'s slide-away first and called
  `on_dismiss` only once the page was off screen, so Escape and a press
  outside closed nothing until the animation had run (the palette tests,
  which look one frame later, saw no close). Escape, back and a press on the
  scrim now reach `on_dismiss` at once; the modal slides away only when its
  owner closes it after such a request, lingering inert (it takes no
  input) while it goes, and stays put if the owner keeps it open, so a
  "discard changes?" answer, or a plugin's `DialogDismissal`, decides first.
  `BackSlide` gained `open: Option<Prop<bool>>` and `on_gone` for this. The
  command palette stays `ModalMotion::Still` by choice, not as a
  workaround: a palette vanishes on Escape rather than sliding away.
- **Checked, no change.** **Escape and a modal with nothing focusable.**
  Opening a trapping overlay moves the focus out of what it blocks, and
  Escape with no focus goes to the top trapping layer, so such a modal hears
  Escape (test `a_modal_with_nothing_to_focus_hears_escape`, which also
  opens one from a popover). block-editor-beui's `PluginDialog` wants the
  keyboard because the plugin's own fields need the keys, and handles Escape
  itself because keys a plugin holds never reach the shell; neither is a
  workaround for beui.
- **Done.** **Dialog's sheet rule matches `Anchored`'s**: it samples
  `touched_by_finger()` when it opens, so a pen opens the card. `use_touch()`
  and `Document::watch_touched` had no other user and are gone.
- **Done.** **Search lists fit their rows.** `SearchList`'s `list_height`
  is a maximum: `Select` and the command palette are as tall as their rows
  up to it. An `Offset` (so a `Scroll`) measured with no limit along its
  direction, as a `Percent` child of a box sized by its content is, now
  measures its content instead of nothing, which gives the picker's full list
  on a phone (a fitted sheet) its rows.
- **Done.** **Subtree opacity.** The base `Fade` takes `opacity` (1 by
  default) besides `edges`: the mask's `Fade` carries it through the wgpu
  shader, the DOM renderer (CSS `opacity`), and paintings (`paint_format::Fade`
  gained a defaulted `opacity`). An opacity change damages the fade's
  rectangle. The level OSD, the dock's drop highlight and drag preview, and
  the split button's divider use it with colour roles. It multiplies each
  shape's coverage, so overlapping shapes inside show through each other.
- **Done, in the picker.** **A plugin dialog's square corners.** A `Below`
  region is composited under its parent's whole surface, so its square
  corners showed through the scrim past the card's rounding. beui has no
  rounded clip and the region is a GPU drawing, so the picker insets the
  creation `Subregion` by `Space::Sm`, enough for `Radius::Lg`; the plugin's
  background matches the card, so the inset doesn't show. A rounded clip
  through the protocol is the general fix if more cards host regions.
- **Done.** Small ones: unstyled `Choice` has no appearance default left
  (`spacing` is required); styled `Scroll`'s `max_length` is a `Size`;
  `Selectable` takes `caret_width` (styled passes the text area geometry's,
  as terminal-view does); the dock's drop marker is a `Stroke`; painted
  corner radii are fitted to their rect, so `Radius::Full` paints as half the
  side.
- **Done.** **App test races.** A driven block-app's type modules used the
  wall clock, so whether two typed words were one undo step depended on how
  long the machine took, and `drive pause` was invisible to them;
  `be_types_host::hold_clock` now holds their clock while the app's is
  stopped. The checklist test measured `#checklist.add` before the plugin
  had drawn it; it waits for it first.
- **Checked, no change.** beui-demo's saved app preference layer is set by
  the toolbar's theme select and the Animations page's "Disable animations"
  box, so it keeps saving it.

## 10. Canvas text on the styled text area, edited in its scaled item

Canvas text is the last import of `unstyled::` outside beui
(`infinite-canvas/src/app/text.rs`). It already draws with styled's look and
colours. What keeps it on the unstyled `TextArea`:

- **Text being edited can't stay in its scaled item.** Read-only canvas text sits
  in a `scales=true` canvas item: it is laid out at its own size and kept crisp
  by the raster scale (pan_and_zoom.md; beui.md "layout and raster"). A scaled
  item takes no pointer input, so the text being edited is moved into an
  ordinary item and fakes the zoom with `font_size = size × zoom`. That can
  wrap differently from the read-only text, lays the text out again on every
  zoom while editing, and needs a font-size prop that nothing else wants.
- **The styled text area doesn't take what the canvas needs**, and always
  shows a find bar.

Changes:

1. **Scaled spaces take input.** Map pointer input through a scaled space's
   scale as well as its translation: presses, drags, hover, caret placement and
   hit tests, selection handles, the IME and on-screen keyboard area, the
   anchors of overlays opened from inside (text menu, completion menu), and
   reveal/scroll-into-view. Update pan_and_zoom.md, which says a scaled item
   takes no pointer input.
2. **`styled::TextArea` gains:**
   - `color: TextColor`, the text colour, with a placeholder derived from it;
   - `read_only` (shown normally, not editable, not reported as disabled),
     kept separate from `disabled`;
   - `focused` and `line_numbers`;
   - `on_content_size`, for a caller that resizes around the text;
   - `find: bool`, default on. Off means no find bar, and its find actions
     aren't registered;
   - `size` for the body text size (`TextSize`, `Custom` for content).

   There is no font-size or scale prop: zoom comes from the scaled space.
3. **Canvas text is a `styled::TextArea`** in its scaled item while edited and
   while read-only, with `find=false`. It gains the text menu and the emoji
   completer, and its wrap no longer changes when editing starts.

## Order

1. §1's `Modal` and `Dialog`, then the §3 moves that depend on
   them (`KeepChanges`, `LockScreen`). Also the moves that are independent:
   `CrashNotice`, `StaleNotice`, `ThemeSelect`, `MotionSelect`,
   `PreferencesSection`, `Bordered`.
2. §4.4 (`Progress`), then the linux-desktop OSD card.
3. §4.5 and §4.6 (buttons and rows), together with §2's tooltip change.
4. §4.2 (the search list), then the launcher move in §3.
5. §4.1 (the anchored surface), which `Select` from step 4 then sits on.
6. `beui-dock` and `beui-keyboard`, then §2's action bar and §4.3's inspector
   change.
7. §1 `Toasts`, §2 tree menus and TextInput, §1's low-priority items.
8. §5, then §6.

§8 is independent too. Do it before §7's migration, so callers are touched
once for both.

§7 is independent of the rest. Make its token types and the styled `Frame`,
`List` and `Grid` early, before §6. That way crates moved onto styled in §6
are written in tokens once, and nobody goes back to change them. Then migrate
the remaining callers one crate at a time.

Paintings may change. Several steps change how things look and work, not
only the code: spacing that snaps to the scale, text that wraps by default,
merged components. Check each changed painting looks right before accepting
it. Each step adds or updates paintings for what it changes. It also keeps
beui-demo in step: removed components leave the demo, and new ones join it.
Update guides/beui.md where it describes a component this plan changes.
