# Ancestor shortcuts across apps

Depends on: none (the host palette is done; guides/apps/editor.md, Actions and
the palette).

Set aside until the user decides. Keys go only to the focused instance, which
runs its own actions. A containing app's shortcut (the workspace's Ctrl+P while
a canvas has the focus) needs the host to try the ancestors' chords when the
focused instance leaves a key unhandled. Two ways:

1. **Apps report whether they handled each key**, and the host retries the
   chord on each ancestor in turn. A round trip per key, limited to keys with
   Ctrl or Alt so plain typing never waits; but async retries can reorder later
   keys, and it changes the "one input path" in guides/beui_keyboard.md.
2. **Extend intercepted keys to ancestors.** An instance's intercepting actions
   already name chords the host offers it ahead of whatever is focused
   (`Document::intercepted_keys`, `Input::InterceptedKey`), but be-host takes
   them only from the shell's frame. Taking them from every ancestor of the
   focused instance needs no round trip, at the cost that an ancestor's
   intercepted chord always wins over the focused app.

The choice needs the user.
