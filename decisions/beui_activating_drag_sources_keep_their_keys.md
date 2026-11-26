# A drag source that activates keeps Space and Enter; it moves with Ctrl+Shift+arrow or Move

A focusable `Interactive` given `drag` that also activates (`on_activate` or
`on_activate_change`: a dock tab, a row button) keeps Space and Enter for activating, as
every button does (guides/beui_keyboard.md). From the keyboard it moves one place with
Ctrl+Shift and an arrow, lifted, stepped to the nearest place that way and dropped at once,
the common way to reorder tabs; and Move in the command palette, offered while it has the
focus, starts a whole keyboard carry (the arrows step, Space or Enter drops, Escape or Tab
puts it down). A source that does not activate (a drag handle, a row with no action) also
lifts that carry on Space.

## Why

A source that activates is a button first: someone pressing Space on a tab expects it
chosen, and a screen reader announces it as a button. Taking Space away to lift would make
the same control answer Space differently depending on whether it is draggable. Ctrl+Shift
and an arrow is what tab bars already use to move a tab, and needs no mode; Move reaches
the full carry, with every place and its announcements, through the palette every editor
has.

## Rejected

- **The ARIA grab pattern on every source** (Space lifts, Space drops): right for a pure
  drag handle, which keeps it, but on a source that activates it takes the key that
  activates.
- **Ctrl+Space to lift:** a chord nothing else uses for it, so nobody would find it; and
  Ctrl+Space is the input method switch on several systems.
