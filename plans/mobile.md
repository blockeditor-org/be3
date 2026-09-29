# Phone layout

A plan to give the app a real phone layout, based on the interactive mockup
at https://claude.ai/artifact/XSHy6PFohaMJpJcHJTgazf. Not implemented yet.
Items marked **(ask)** need a decision from the user during the work.

Desktop and tablet layouts stay as they are. Everything below applies only
under the phone breakpoint.

## The target

- **One screen at a time.** Files is a full-screen page you move through one
  folder at a time: a back button and path in the header, a Recent row at the
  top level, search, and a floating "New" button. Opening a file replaces the
  page, and back returns to the folder it came from.
- **The title bar is the file switcher.** Each open file has an app bar:
  back, the type icon, name and path, a count of open files, and a more
  button. Tapping the name or the count opens a sheet that shows the open
  files as cards, with close, New and "Browse all files".
- **One sheet to create a file**, holding the name, the type and the location
  (folder chips). It opens from Files, the switcher and a file's more menu,
  and the new file opens ready to edit.
- **Editors put their controls at the bottom.**
  - The text editor's formatting bar docks above the keyboard only while
    you are editing.
  - The canvas has a tool dock (select, pen, rectangle, sticky, text, embed,
    undo), a zoom pill, and a selection bar above the dock (colours,
    duplicate, bring to front, delete, open for embedded blocks).
  - The inspector opens as a sheet on demand.

## What already exists

- **Safe area and keyboard.** `BeuiView.onApplyWindowInsets` sends system
  bars, cutout and IME insets to `nativeInsets`. `safe_rect` in
  `beui-core/src/app.rs` lays the whole app out inside them. So a panel
  anchored to the bottom already sits above the keyboard.
- **Knowing it is narrow.** `narrower_than(w)` / `shorter_than(h)`
  (`beui-components-unstyled/src/container.rs`) and, inside editors,
  `narrow_chrome()` / `sheet_open()` (`block-editor-beui`).
- **Sidebars as sheets.** `ChromeRoot` / `Sidebar` in
  `block-editor-beui/src/chrome.rs` fold sidebars into a styled `Sheet` when
  narrow. The narrow `Toolbar` scrolls sideways and gets the "Sidebar" toggle.
- **The canvas's phone pieces.** It already has a phone dock (`ToolDock`) and
  a floating `SelectionBar` in `infinite-canvas/src/app`. It has finger
  drawing, finger box-select, and a phone test (`phone()`, 390×760).
- **Touch input.** Long-press opens context menus (`LONG_PRESS_DELAY`).
  `touch_drags` on `ClickCatcher` / `Drag`, fling in `Scroll`, and 2- and
  3-finger taps for undo/redo (`beui_frame.rs`).
- **Opening and switching files.** A plugin can open or switch files with
  `EditorMessage::OpenBlock` / `ShowBlock` (`host.open_block_via`,
  `Shell::show_in_shell`).

## Gaps found

- Three breakpoints disagree: 640 (`block_editor_beui::NARROW_WIDTH`), 700
  (workspace-ui `COMPACT_FILES_WIDTH`) and 720 (`theme::NARROW_WIDTH`).
- `Sheet` stops are the fixed constant `SHEET_STOPS`, and it has no scrim, so
  it cannot act as a modal.
- Open tabs are not saved: `WorkspaceUiContent` is empty. Nothing records
  recents, and `BlockInfo` has no timestamps.
- The file tree's "+" row button (`AddChild{shown: face.hovered}`) only
  shows on hover. Its rows are 22px and the styled tree row doesn't set
  `touch_drags`.
- The Add block dialog (`block-app/src/ui/picker.rs`) is a 640px `Dialog`.
  It has no name field and doesn't choose a location; the requesting plugin
  places the block afterwards.
- The host's `StatusBar` (save state, More menu) is outside the workspace-ui
  plugin, and a plugin can't open host menus.
- The text editor is one continuous `TextArea` over the markdown, and its
  toolbar is a top band. There are no phone-size text tests.
- The canvas has no sticky note, and no embed or undo in the dock. The zoom
  controls live in the top toolbar. `SelectionBar` floats at the top and has
  no bring to front, although `CanvasCommand::Reorder` exists.

## Phases

Each phase is shippable alone and adds phone-size tests. Use
`with_size(Vec2::new(390.0, 800.0))` and `with_scale_factor(2.0)` with
`.paint` snapshots, per `guides/testing_a_gui.md`.

### 1. Foundations

- **One phone breakpoint.** Make `theme::NARROW_WIDTH` the only one (pick one
  value, 700) and delete the other two constants. Point `ChromeRoot`,
  workspace-ui's compact check and `ResponsiveTabs` / `Stack` at it.
- **`Sheet` upgrades** (`beui-components-styled/src/sheet.rs`):
  - a `stops` prop, defaulting to today's `SHEET_STOPS`;
  - a `modal` prop that draws a scrim, closes on a scrim tap, and keeps the
    existing `BackHandler`;
  - a `fit` stop that sizes the sheet to its content, for short action
    sheets.
- **A shared `BottomDock` in `block-editor-beui`.** Generalise the canvas's
  `ToolDock`: a `Floating edge=Bottom` pill for icon buttons with pressed
  states, hidden while an editor sheet is open. Editors share it so the
  docks look alike.
- **Save workspace state.** Give `WorkspaceUiContent` the open tabs (block
  id, type, `opened_via`), the active tab and a recents list. Cap recents at
  about 20, updated on `report_focus`. No backwards compatibility needed.

### 2. Phone shell (workspace-ui)

- In `WorkspaceBody`, when narrow, render a new `PhoneShell` instead of the
  `DockArea`.
  - It is a two-level page stack: the Files page, or one open file.
  - The same `Workspace` state drives both layouts, so resizing across the
    breakpoint keeps the same files open.
  - `set_files_compact` goes away.
- **App bar.** It holds:
  - back, to the Files page at the file's folder, using `opened_via`, which
    already holds the path;
  - a title button with the type icon, name, and path in small type;
  - the open-file count;
  - a more button.
  Android back does the same as the back button, through `BackHandler`.
- **Switcher sheet.** A modal `Sheet` with a two-column grid of open files.
  - Each card shows the type icon, name, path and a close button.
  - A preview is optional. Use the block's thumbhash if present; real
    thumbnails need the plugin texture path and can come later.
  - Actions: New (phase 4) and "Browse all files".
- **More sheet.** It replaces the per-panel `StatusBar` on phone. It holds
  the `menu.rs` actions (Open, New file here, Rename, Share, Unlink,
  Delete), plus the Parents / References / Backrefs lists as rows that open
  a list sheet. `ArtifactBar` and `LinkedBar` shrink to a one-line banner
  under the app bar.

### 3. Files page (file-tree)

- Add a drill-in mode to the file-tree editor, used when narrow, so the
  file tree still owns rows, menus, drag and picking.
  - It shows one level: `BlockQuery::Roots` at the top level, and
    `References(id)` for a folder.
  - The header shows a back button and the path, built from
    `RowKey::Block(Vec<Uuid>)`, which already holds the full path.
  - Rows are 64px: a type tile, the name, a subtitle (the child count for a
    folder, otherwise the type), and an always-visible more button that
    opens the same actions as the context menu. Long-press still opens it.
  - A floating "New" button opens the creation sheet with the current folder
    as its location.
  - Recently Deleted stays as the last row at the top level.
- **Recent row.** A horizontal row of cards from phase 1's recents. The file
  tree reads them from the workspace-ui block, or the host exposes them.
  **(ask)** which of the two.
- **Search.** A field that filters by name across the workspace. The current
  queries only walk the tree, so either keep a client-side index of blocks
  the tree has watched, or add `BlockQuery::Search(String)` on the host.
  **(ask)**; `BlockQuery::Search` is the better answer if workspaces get
  large.
- **Reparenting on phone.** Use "Move to…" in the more sheet, which opens a
  folder picker sheet, rather than dragging.
- **Header.** On phone, the workspace switcher, the save state and the
  account button move here (phase 6).

### 4. New file sheet (host picker)

- **On phone, render as sheets.** `ChooseDialog` becomes a modal `Sheet` at
  the `0.9` stop. The type grid moves to 2 columns of wide tiles (icon,
  name, one-line description). `Tabs` becomes `ResponsiveTabs`. `CreateDialog`
  (type options) becomes a sheet too.
- **Add Name and Location to the picker.**
  - The pick request (`HostRequest::PickBlock` / `BlockFilter`) gains an
    optional default parent and a flag to show the location row.
  - The reply (`BlockPick`) returns the chosen name and parent.
  - `finish_creation` calls `set_name` and `set_parent`, so the requester no
    longer places the block itself when a location was chosen.
  - These are plain data fields, so the protocol stays framework-independent.
- **After creating, open the file with a text cursor placed.** For text,
  the cursor lands on the first empty line; for a checklist, in a new empty
  item.

### 5. Editors

**Text (`editors/text-block`)**

- **Keep the continuous `TextArea`; don't switch to per-block editing.** The
  mockup's tap-to-edit feel comes from tapping to place the caret, which
  `tap()` already does.
- **Formatting bar.** When narrow:
  - hide `EditorToolbar`'s top band;
  - while the surface has focus (an `ImeCursor` is published), show the
    `MarkdownControls` in a `BottomDock`-style bar above the keyboard;
  - order the buttons for a phone: heading menu, bold, italic, bulleted
    list, checklist, link, then an overflow menu for the rest;
  - end with a Done button that clears focus, which also hides the keyboard
    via `set_keyboard`.
- **Code files.** Language, Indentation and Find move into the more sheet
  (phase 2) on phone.
- **Tests.** Add phone-size tests: the bar appears on focus and goes on
  Done, and bold wraps the selection.

**Canvas (`editors/infinite-canvas`)**

- **Hide the top `CanvasToolbar` when narrow.**
  - `ZoomControls` becomes a floating pill at the top right: −, %, +, and
    fit, reusing `editor.zoom_at` / `fit`.
  - `ActionsMenu` moves into the app bar's more sheet. That needs a way for
    an editor to add items to the shell's more sheet. **(ask)** whether to
    add this to the protocol, or keep a small ⋯ button on the canvas.
- **`ToolDock` becomes the shared `BottomDock`.** Its buttons: Select, Pen,
  Rectangle, Sticky, Text, Block (the existing `open_block_picker`), then a
  divider and Undo. Undo uses the framework's `history()` / `step()` that
  the top bar uses. Artboard and Line move into a long-press menu on the
  rectangle button.
- **Sticky note.** Either a new `Sticky` entity kind in
  `be-block/src/canvas.rs` (fill, text, auto-wrap), or a preset rectangle
  with centred wrapped text. **(ask)**
- **`SelectionBar` on phone.**
  - Anchor it at the bottom, above the dock.
  - Add bring to front (`CanvasCommand::Reorder`), and Open for an embedded
    block (`OpenBlock`).
  - Add a "tune" button that opens the inspector sheet. It replaces the
    "Sidebar" toggle, so the inspector is reached from the selection.
- **Text editing.** Tapping an already-selected text object starts editing
  it.
- **Tests.** Extend the existing `phone()` tests: dock tools, the selection
  bar at the bottom, zoom pill fit, and a sticky created with a finger.

**Other editors.** They get the phone shell and sheets automatically. Audit
the ones with sidebars (image, database, presentation, video editor,
calendar) at 390px, and add a `BottomDock` where a top toolbar holds the main
tools.

### 6. Host chrome on phone (block-app)

- **Hide `StatusBar` when narrow** (`ui/workspace.rs`). Its contents move to
  the Files page header.
  - The save state becomes a small icon. It needs the host to push the save
    state to the workspace-ui / file-tree editor as host state, like the
    other pushed state.
  - The account button opens the More menu. Add a plain-data
    `EditorMessage::ShowAppMenu { anchor: Rect }` that the host answers by
    showing its More menu as a sheet. The protocol stays
    framework-independent.
- **Onboarding screens** (`AccountsScreen`, `WorkspacesScreen`). Check them
  at 390px. They already use a centred `Column`, so this is likely only
  spacing.
- **Edge to edge (optional).** Today `safe_rect` crops the app, so the
  status and navigation bar areas show the clear colour. Pass the insets into
  the `Context` as well. Then the root background and sheets can extend under
  the system bars, and the app bar and docks add the inset as padding.

### 7. Checking on a device

For each phase, after `//:verify`:

- Run `./scripts/buck run //crates/block-app:android -- --install` and try
  the four mockup flows on a phone:
  - edit text with the keyboard up;
  - switch files;
  - create a file in a folder;
  - draw, move and zoom on the canvas.
- Check back-button behaviour at each level.
- Run `//crates/block-app:web` at phone width to check that the same layout
  appears in a narrow browser.

## Order and size

Phases 1 and 2 unlock everything else. Phases 3–5 can then run in parallel,
and 6 is small. The largest pieces are:

- the picker protocol change (phase 4);
- the more-sheet contribution question (phase 5, **(ask)**);
- sticky notes, if they become a new entity kind.

## Status

All six phases are implemented. Where the result differs from the plan:

- Recents live in the WorkspaceUi block, so they sync across devices.
- Search only shows a "coming soon" screen.
- There is no sticky note. Artboard and Line moved from the canvas dock to
  the More sheet.
- Editors add to the More sheet with `bar_item`, since the whole top bar
  belongs to the plugin. On a phone, the side panel is opened from the More
  sheet or the canvas selection's tune button.
- The save state is shown in the app menu sheet, not pushed to plugins. The
  app menu opens through `BlockCommand::AppMenu` from the account button on
  the Files page.
- Onboarding screens and the web build at phone width have not been
  checked.
- The phone shell is no longer a separate tree: the workspace draws its
  dock in stacked mode, so the tabs and the editors in them survive
  crossing the breakpoint, and the switcher lists the dock's recent tabs.
  The system back gesture reaches plugins' BackHandlers.
