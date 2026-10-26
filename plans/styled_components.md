# Styled component cleanup

An audit of `beui-components-styled` against `beui-components-unstyled` found
styled components that carry behaviour, components that don't belong in beui,
and parallel components that should be one. This plan is the agreed list of
changes. Not implemented yet.

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

- **`unstyled::Modal`**: an Overlay, a `BackSlide`, a scrim that fades with
  presence, and dismiss handling. `styled::Dialog` and `styled::Fullscreen`
  (`dialog.rs`, `fullscreen.rs`) copy this today. `ModalSheet` and the launcher
  overlay should share it where they can.
- **`unstyled::Dialog`** on top of `Modal`: Enter confirms unless the action is
  destructive or disabled (`DialogActions::enter`), Dialog or AlertDialog
  accessibility with the title as its label, confirm and cancel callbacks.
  Make it placeable in a given rectangle and not only centred, so block-app
  can build keep-changes and the lock screen out of it (see §3).
  `DialogSurface` is `pub(crate)` today, so nothing outside styled can do
  that.
- **`unstyled::Toasts`**: the stack, each toast's expiry timer, the rule that
  sticky and danger toasts don't expire, edge placement, the overlay that lets
  input pass through, and an accessibility live region. Toasts have no role
  today. Keep `Toast`'s notification fields (image, actions, activates,
  sticky). Toasts are generally useful with them.
- **Done.** **Indeterminate progress**: the phase animation, the reduced-motion check and
  the busy accessibility node from `styled::Spinner`. This becomes part of the
  merged `Progress` (§4.4). It is `unstyled::Progress`, whose handle carries
  the clamped `value` and the sweep's `phase`.
- **`EveryScreen`**: one copy of something per screen, from `use_screens()`,
  each given its screen's rectangle relative to the bounds of all screens.
  `KeepChanges`, `LockScreen` and `LevelOsd` each repeat this. All three are
  moving out of beui (§3), but they still need it, in two different crates.
- **Low priority:** an unstyled `Table` for the Table, Row, ColumnHeader,
  RowHeader and Cell accessibility nodes that `styled::Table` builds. An
  unstyled `ColorInput` for the preview state and the popover plus hex-field
  composition. Most of that is already in `HexText` and `Popover`.

## 2. Move behaviour out of styled components

- **An unstyled action bar.** `DockMenu` (`beui-dock/src/styled.rs`, about lines
  690-850) decides which actions are pinned to the bar and which overflow into
  a "More" menu, by available room (`pinned`, `unpinned`, `BarRoom::shows`,
  `title_action`). Make that an unstyled action bar the dock uses. Any other
  toolbar can use it too.
- **Tree row menus.** `TreeRowChrome` and `RowMenuButton` (`styled/tree.rs`
  240-357) wire each row's context menu. The More button finds its own
  rectangle with `placements().rect` and opens a context menu at that point.
  Make per-row menus a feature of `unstyled::Tree`, and make the More button a
  menu button anchored to itself.
- **Done.** **Tooltips on icon-only buttons.** "Show the tooltip only when the button is
  icon-only and its popup is closed" is repeated in `MenuButton`, `Popover`,
  `ToggleButton` and `IconButton`. `ButtonHandle` already carries `tooltip`,
  so let the unstyled button decide when it shows. `unstyled::Button` and
  `unstyled::Toggle` take `icon_only` (and the button `expanded`), and the
  handle's `tooltip` is empty unless the button is icon-only and its popup is
  closed. The styled faces wrap themselves in a `Tooltip` of that text, since
  the bubble is a look; `SplitButton`'s arrow, the date picker's and the
  colour input's triggers lost their copies too.
- **Dialog Enter handling**: moves with §1's `unstyled::Dialog`.
- **Done.** **Spinner animation**: moves with §1's indeterminate progress.
- **TextInput clear and copy.** `styled::TextInput`'s clear and copy buttons
  (`copy_text`) are behaviour. Expose them from `unstyled::TextInput` as
  handle callbacks, and leave only the buttons' look in styled.

## 3. Move components out of styled

- **`KeepChanges` → block-app.** It is a timed AlertDialog and only block-app's
  display settings use it. Once §1's Dialog and `EveryScreen` exist, it should
  be a few lines in block-app. Delete `styled::KeepChanges` and its demo page;
  move its tests with it.
- **`LockScreen` → block-app.** Only block-app uses it (`ui/lock.rs`), and
  nothing else ever would. Build it in block-app from styled pieces and §1's
  `EveryScreen`.
- **Done.** **`LevelOsd`.** Its bar becomes `Progress` (§4.4). What's left is an OSD card
  used only by linux-desktop (`app/media.rs`), so move it there. It is in
  `app/osd.rs` now, with its tests in linux-desktop's; it still lays out one
  card per screen itself, so switch it to `EveryScreen` once that exists.
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
  still its own face: build them on §4.6's row once it lands.
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
the highlight, which skips disabled rows, Up/Down/Page Up/Page Down,
scroll-to-row, Enter, clicks and hover. `rank`, `substring_score` and
`fuzzy_score` are the ranking helpers. `Select` and `CommandPalette` (and
through it linux-desktop's launcher) are built on it; their trigger,
popup, panel, search field and rows stay theirs. Home and End are no
longer taken from a Select's search field, so they move its caret; they
still open a closed Select at its first or last option. Filtered-out rows
are no longer built, so `SelectApi::option_button` returns an `Option`.

The completion menu doesn't fit, so it keeps its own list. Its query is a
span of the text area's own text, not a field of its own; its keys reach the
text area's key map while the caret stays in the document (and Tab
completes); its overlay passes input through instead of trapping the focus;
and the `Completer` returns its matches already ordered. Sharing would mean
splitting the search list's highlight and keys away from its field for one
user that has no scroll, no disabled rows and no Page keys.

### 4.3 Tabs

- **Merge `ResponsiveTabs` into `Tabs`.** `Tabs` always collapses into a Select
  when it's narrow, the way a menu always adapts to touch. Today only the
  demo and its own test use `ResponsiveTabs`, and the real uses of `Tabs`
  never collapse.
- **Move the beui-inspector tool tabs to dock tabs.** Beui/Comp/A11y/Perf/Sim
  (`beui-inspector/src/panel.rs:259`) are independent panels, and side by side
  is useful. This needs `beui-dock` (§3).
- **Later, maybe: the inspector's App/Inspector bar.** That compact-mode bar
  (`panel.rs:405`) is the dock's phone layout with the app as a pane. Revisit
  it once the inspector is a dock.
- **The block-app invite role is not tabs.** Editor/Administrator
  (`block-app/src/ui/dialogs.rs:596`) is a form value. It moves to the
  segmented control below.
- **Keep these as `Tabs`:** the input-remaps Editor/TOML views
  (`editors/settings/src/input_remaps/ui.rs`) and the `DateTimeField` paged
  Date/Time picker.
- **Add a styled segmented control** over `Choice`'s `Radio` kind, so it has
  radio semantics. Tabs switch a panel; a segmented control picks a value.
  Move calendar's Day/Week/Month, be-launcher's Open/Closed and block-app's
  invite role onto it. Today they use `Tabs` and report tab roles with no tab
  panel. It collapses into a Select when narrow, as `Tabs` does. Add it to
  beui-demo.

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

These are covered by §1 and §3: `Dialog` and `Fullscreen` sit on
`unstyled::Modal`, and block-app's keep-changes prompt is a `Dialog` with a
countdown.

## 5. Remove appearance defaults from unstyled

Following the first rule, these become required props and styled passes them:

- `TextInputStyle::default()` (white text, fixed placeholder, selection and
  caret colours)
- `TextAreaColors::DEFAULT`
- `SHEET_STOPS` as the default `stops` and `rest` of `Sheet` and `ModalSheet`
- the text area's `BODY_SIZE` font size and `PADDING`
- `TimeList`'s 32px `row_height`
- the empty `<List/>` placeholders that are used when a trigger, row or option
  renderer isn't passed (`Select`, `CommandPalette`); make these required

Look for others while doing this, and list any you find here.

## 6. Get code outside beui onto the styled layer

About 29 crates import `unstyled::` directly. The biggest are block-shell (13
files), linux-desktop (12), settings (10), block-editor-beui (7),
infinite-canvas (6), database (6) and be-launcher (6). Audit each import: it
is either a styled component that doesn't exist yet, or a custom face that
should become a styled component. This is the largest item. Do it after §1-4,
which remove some of those imports on their own.

## 7. Styled components take design tokens, not raw values

Text already takes tokens: `TextSize`, `Weight`, `Family`, `TextColor`. The
rest of the styled layer still takes raw values. Outside beui, about 205 files
use styled. Between them they have about 740 `spacing=`, 180 `padding=`, 33
`Radius::custom`, 56 `ColorRole::Custom` and 17 `TextColor::Custom`, plus
about 190 private `*_SPACING`, `*_PADDING`, `*_INSET` and `*_MARGIN` consts.
Styled has about 190 of those consts of its own.

- **Spacing tokens.** Add a `Space` scale, named like `Radius` and `TextSize`.
  It resolves through `Density`, so compact and comfortable layouts come from
  the tokens instead of from each component (`Tree` has its own `Metrics` for
  this today). The values in use are 0, 2, 4, 6, 8, 10 and 12, then a long
  tail of 14, 16, 18, 20 and 24. Use the steps 0, 2, 4, 6, 8, 12, 16, 20 and
  24. Snap 10, 14 and 18 to whichever neighbour looks right in each place,
  and accept the paintings that change.
- **Styled `Frame`.**
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
- **Styled `List` and `Grid`.** Styled re-exports the base `List` and `Grid`
  unchanged, so `spacing` is raw. This is the biggest case, with about 740
  uses. Add styled versions whose spacing takes `Space`, and stop
  re-exporting the base ones. Do the same for `Gap` and `Spacer` widths in
  styled `Text`.
- **Complete `ColorRole`.** `ColorRole::Custom(theme.dim.get())` exists
  because the theme has colours that have no role: `danger_fill` and its
  hover and active, `danger_soft`, `on_danger`, `dim`, `scrim`, `scrim_soft`,
  `scrim_strong`, `control_outline` and `knob_outline`. Add them, so code
  never reads a theme colour to hand back as a custom one. Do the same for
  `TextColor`.
- **Colours from content.** After that, `ColorRole::Custom` and
  `TextColor::Custom` stay as escape hatches. They are for colours that come
  from content: a user's tag colour, a remote cursor, a presentation
  background, a chart series, a colour swatch. Don't use them for a colour
  the theme has or should have.
- **Component props in pixels** move to tokens:
  - Size tokens or a named size: `Dialog`'s `width`, `Popover`'s
    `max_width`, `MenuPopup`'s `min_width`, `Calendar`'s `width`,
    `Select`'s `width`, `Spinner`'s `width` (merged into `Progress`) and
    `Separator`'s `length`.
  - `Space`: `Tree`'s `spacing` and `padding`, and `styled::Stack`'s
    `spacing`.
- **Inside styled too.** Rewrite the private consts in styled (button and
  row paddings, gaps, focus offsets) in tokens. Tokens that styled itself
  doesn't use aren't real.
- **The types enforce it.** Once styled props take token types, a raw `8.0`
  doesn't compile, and no lint is needed. Unstyled and base components keep
  taking concrete values, as the design principles say.

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

- **Actions are data.** An action has a label, a role (`Primary`, `Cancel`,
  `Destructive`, `Secondary`), `disabled`, `busy` and `on_click`. The modal
  decides placement, order, style, focus and keys. This replaces `confirm`,
  `cancel`, `destructive` and `extra_actions`. It lives in §1's
  `unstyled::Dialog`, and styled paints it.
- **Two kinds**, as GNOME's libadwaita has them.
  - **An alert** is a decision: leave, discard, delete, approve. Its actions
    go in the footer and its role is AlertDialog. Enter does the primary
    unless it's destructive. A destructive primary is red, and focus starts
    on Cancel.
  - **A dialog** is content: settings, members, inspect, about. It has a
    title row with a close X, and a footer only when there's something to
    confirm. Every modal has a visible way out.
- **Wide layout.** The footer is right-aligned: secondary actions on the left,
  then Cancel, then Primary (the macOS and GNOME order). It stays put while
  the content scrolls.
- **Narrow or touch layout.** A dialog becomes a sheet on its own, the way
  menus do, so the picker's hand-written switch goes. An alert stays a
  centred card. In both, the footer is pinned to the bottom with full-width
  buttons, primary first, and stays above the on-screen keyboard.
- **Busy actions.** `busy` disables the action and shows progress in its
  button. This replaces the hand-placed spinners.
- **Confirmations inside a dialog are nested alerts.** Members' Remove/Keep
  becomes an alert with a red Remove.
- **Row and control actions stay in the content**: a member's role, removing
  a member, share's Add. Only the modal's own actions go in the footer.
- **Convert every modal listed above**, the dock's Rename included.

### The dialog's inside is one component that plugins use too

Split the dialog into an outer part and an inner part:

- **The outer part** belongs to whoever shows the dialog: placement, scrim,
  presence, the card or sheet frame, back and dismiss.
- **The inner part** is a styled `DialogBody`: the title row with its close
  X, the scrolling content, and the pinned footer with role actions, laid out
  wide or narrow. `Dialog` and `ModalSheet` use it.

A plugin opened as a dialog gets the whole area inside the frame, with no
padding and no shell title bar. A beui plugin builds its contents with the
same `DialogBody`, so it defines its own buttons with the code beui uses for
its own dialogs. The plugin protocol stays free of beui: it carries only
what any framework needs.

- **Back and dismiss go to the plugin first.** Back already reaches a plugin
  as input. Escape, back and a press on the scrim reach the plugin, whose
  `DialogBody` runs its Cancel action. That way a form with unsaved changes
  can ask before closing. The plugin asks the host to close it. The host
  closes it itself only if the plugin doesn't answer.
- **Size.** The plugin reports the height it wants, so a card or sheet fits
  its content, and the host passes back the size it got. Reuse the
  intrinsic sizes the protocol already has for child placements if they
  fit.
- **Keyboard and safe area.** The host tells the plugin the bottom inset (the
  on-screen keyboard, a phone's gesture bar), so the pinned footer stays
  visible.
- **Window dialogs** keep a frame but no title bar. Moving one by dragging
  its title row needs the plugin to report that row as a drag region. Check
  whether anything moves dialogs today before adding that.
- **Other frameworks.** A plugin written with another framework (egui, say)
  gets the same area and messages and draws its own version. Only beui
  plugins match the shell's look exactly, which follows from keeping the
  protocol free of beui.

## Order

1. §1's `Modal`, `Dialog` and `EveryScreen`, then the §3 moves that depend on
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
