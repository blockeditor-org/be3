# Reading the screen with `drive`

## The tree

`drive tree` is the accessibility tree, one node per line, indented under its parent:

    Window "Block" at 0,0 size 1100x720
      Dialog "Invite member" at 360,180 size 380x300
        TextInput "Email address" focused at 380,260 size 340x36
        Button "Send invitation" at 380,420 size 116x36
          Label value="Send invitation" at 392,430 size 92x15
      Pane "Checklist" at 253,51 size 837x608
        CheckBox "buy milk" toggled=True at 307,307 size 624x18
        Slider "Load" value=0.8 at 291,476 size 771x20

Coordinates are pixels from the window's corner. `offscreen` marks a node scrolled or clipped
out of view and `focused` the one with keyboard focus. In block-app each plugin region is a
`Pane` named after its plugin, holding the nodes its editor describes (guides/adding_a_plugin_editor.md),
so dialogs, the file tree and every editor read like the host's own controls.

Test ids (guides/testing_a_gui.md) are the steadiest way to name a control. `drive tree` shows
them as `#id` after a node's state, and a node with nothing else to show reads as a
`GenericContainer`; `drive ids` lists every one with where it is, the plugins' included.

## Targets

A command that acts on something takes a TARGET:
- `#TEST_ID`, such as `#checklist.add`;
- `X,Y` in the tree's pixels, or `X,Y,WIDTH,HEIGHT` for an area (to `shot`);
- text found in one line of the tree, such as `'"Send invitation"'` or `'Button "Create"'`.
  A match inside another match counts as that one, so a button and its own label are one
  target; when several unrelated lines match, the error lists them.

## Other views

- `drive find TARGET` says where a target is.
- `drive hit TARGET` says what a press there would reach without pressing: the `Interactive`s
  along the hit path, innermost first, each with its kind, rect, test ids, the components it was
  made in and what it handles, then what forwarded input goes to and which overlays the press
  would dismiss. Use it when a press goes to the wrong place or nowhere. It sees the host's own
  document only; in block-app a press over a plugin ends at that plugin region's catcher.
- `drive state` is what the tree does not say: the window's size, scale and focus, whether it is
  fullscreen, the cursor's shape, the pointer and the fingers down, held keys, an input
  method's composition, a back gesture under way, files being dragged in, and any open file
  dialog.
- `drive actions` lists the actions the command palette would offer where the focus is
  (guides/beui_keyboard.md), with their shortcuts, and each plugin's under its `Pane`;
  `drive actions all` lists every registered one.
- `drive shot FILE [TARGET]` writes a PNG of the window, or of one target.
