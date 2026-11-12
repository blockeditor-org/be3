# An action row and a list row stay two components

`ActionRow` is a button that happens to look like a row. `ListRow` is a
selectable item of a list. Merging them into one `ListRow` with `action` and
`danger` was proposed and rejected.

## Why

- Roles differ. An action row is a button; a list row is a list item or option
  with a `selected` state. A merged row would pick its role from its props, so
  one name would hide two behaviours.
- Keyboard differs. A button activates on Enter and Space and can carry an
  `action` hotkey; a list row is walked with arrow keys, selected, and
  activated by `on_activate`.
- Sizes differ. `ActionRow` is touch-sized; `ListRow` is compact by default.
  Merging would make every user of one pass a size to keep its looks.
- `SettingsLink` and `SettingsRow` stay apart for the same reason: a link is a
  whole-row button with a chevron, a settings row holds controls and a menu.

One obvious way to do things means one name for one behaviour, not one name
with props that choose between behaviours.
