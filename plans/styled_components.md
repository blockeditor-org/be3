# Styled component cleanup

An audit of `beui-components-styled` against `beui-components-unstyled` found
styled components that carry behaviour, components that don't belong in beui,
and parallel components that should be one. This plan is the agreed list of
changes. Not implemented yet. Items marked **(ask)** need a decision from the
user before or during the work.

## The rules this plan applies

- **Unstyled has no appearance defaults.** Someone using only the unstyled layer
  can implement any style without inheriting ours. Colours, sizes, paddings,
  breakpoints and placeholder renderers are required props at the unstyled
  layer and are chosen by the styled layer. **(ask)** Behaviour defaults (drag
  threshold, tooltip delay, zoom limits, a slider's 0..1 range) stay allowed.
  Confirm that line.
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
- **Indeterminate progress**: the phase animation, the reduced-motion check and
  the busy accessibility node from `styled::Spinner`. This becomes part of the
  merged `Progress` (§4.4).
- **`EveryScreen`**: one copy of something per screen, from `use_screens()`,
  each given its screen's rectangle relative to the bounds of all screens.
  `KeepChanges`, `LockScreen` and `LevelOsd` each repeat this. All three are
  moving out of beui (§3), but they still need it, in two different crates.
- **Low priority:** an unstyled `Table` for the Table, Row, ColumnHeader,
  RowHeader and Cell accessibility nodes that `styled::Table` builds. An
  unstyled `ColorInput` for the preview state and the popover plus hex-field
  composition. Most of that is already in `HexText` and `Popover`.

## 2. Move behaviour out of styled components

- **An unstyled action bar.** `DockMenu` (`styled/dock.rs`, about lines
  690-850) decides which actions are pinned to the bar and which overflow into
  a "More" menu, by available room (`pinned`, `unpinned`, `BarRoom::shows`,
  `title_action`). Make that an unstyled action bar the dock uses. Any other
  toolbar can use it too.
- **Tree row menus.** `TreeRowChrome` and `RowMenuButton` (`styled/tree.rs`
  240-357) wire each row's context menu. The More button finds its own
  rectangle with `placements().rect` and opens a context menu at that point.
  Make per-row menus a feature of `unstyled::Tree`, and make the More button a
  menu button anchored to itself.
- **Tooltips on icon-only buttons.** "Show the tooltip only when the button is
  icon-only and its popup is closed" is repeated in `MenuButton`, `Popover`,
  `ToggleButton` and `IconButton`. `ButtonHandle` already carries `tooltip`,
  so let the unstyled button decide when it shows.
- **Dialog Enter handling**: moves with §1's `unstyled::Dialog`.
- **Spinner animation**: moves with §1's indeterminate progress.
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
- **`LevelOsd`.** Its bar becomes `Progress` (§4.4). What's left is an OSD card
  used only by linux-desktop (`app/media.rs`), so move it there.
- **`Launcher`.** Its core joins the command palette (§4.2): a styled
  searchable palette with pluggable ranking and rows that show a picture or
  icon and a detail line. Also move `launcher_score` there as one of the
  ranking functions. linux-desktop and be-launcher then build on that
  palette and pass in their icons. The "Run 'query'" fallback row is the only
  other extra. If a palette option for a fallback row covers it, neither crate
  needs a shared launcher crate. **(ask)** if it turns out they share more.
  `plans/command_palette.md` also builds on `styled::CommandPalette`, so keep
  the two plans in step.
- **`CrashNotice` and `StaleNotice` → block-app.** Only block-app uses them
  (`compositor/region.rs`, `surfaces.rs`). Their text is about plugins.
- **`ThemeSelect` and `MotionSelect` → next to the settings editor** (the only
  user, `editors/settings/src/display/appearance.rs`), or into
  `block-editor-beui` beside `LayeredSettingsRow`. Delete
  `PreferencesSection`: only the demo uses it, and `LayeredSettingsRow`
  already does its job.
- **Dock → a new `beui-dock` crate** with an `unstyled` module and a `styled`
  module. Any beui user can then use the dock with their own styles. That is
  about 9,000 lines of unstyled (`dock.rs`, `dock/state.rs`, `docking.rs`,
  `spec.rs`, ...) and about 1,400 of styled. The unstyled dock depends only on
  beui-core, beui-view and unstyled pieces (menus, drag, choice, sheet, scroll),
  so the move should be mechanical. beui-inspector is the precedent for a beui
  crate outside the component crates.
- **Software keyboard → a new `beui-keyboard` crate**, the same shape as
  `beui-dock`. beui itself depends on it (`beui/src/keyboard.rs`), like
  beui-inspector.
- **Delete `Bordered`.** It is `Frame border=Border::DEFAULT radius=…`, and its
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
  (`block-app/src/ui/dialogs.rs:596`) is a form value. It tells screen readers
  "tab" with no tab panel. Use a `RadioGroup` or a `Select`.
- **Keep these as `Tabs`:** the input-remaps Editor/TOML views
  (`editors/settings/src/input_remaps/ui.rs`) and the `DateTimeField` paged
  Date/Time picker.
- **A segmented control.** **(ask)** Calendar Day/Week/Month and be-launcher
  Open/Closed pick a value; they don't switch panels, so they report the wrong
  roles. A styled segmented look over `Choice`'s `Radio` kind would give them
  radio semantics. The invite role could use it too. It's a separate concept
  from tabs ("Tabs switches a panel, a segmented control picks a value"), so
  it doesn't break "one obvious way". If it's added, the narrow-width collapse
  applies to it too.

### 4.4 One Progress

Merge `Spinner` into `Progress`: `value: Option<f32>`, where `None` is
indeterminate. Use §1's indeterminate progress for it. linux-desktop's OSD
card (§3) uses `Progress` for its bar, not a hand-drawn track and fill.

### 4.5 One button, one menu button

- `Button` takes `icon_only` and a size, and `IconButton` goes. `ButtonFace`
  already supports `icon_only`.
- `MenuButton`, `IconMenuButton` and `PictureMenuButton` become one menu button
  whose face is text, an icon or a picture. `IconMenuButton` has one product
  user (database).
- `Popover`'s trigger reuses the menu button's trigger instead of copying it.

### 4.6 One row button

`ListRow`, `ActionRow` (the menu sheet's row face as a standalone button),
`SettingsLink` and Launcher's private row each paint a pressable row with
hover and selection. Make one styled row with the slots they need (leading
picture or icon, label, detail, trailing chevron or check), and rebuild the
four on it.

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

Each step adds or updates paintings for what it changes. It also keeps
beui-demo in step: removed components leave the demo, and new ones join it.
Update guides/beui.md where it describes a component this plan changes.
