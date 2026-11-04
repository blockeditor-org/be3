# On Windows the shell's `focus` window action does not start focus mode

On the Windows host, the shell's `focus` window action (a press on a panel's title bar, the
picker, a tab given the keyboard) does nothing (`win32_map::shell_focus`). Focus mode, which
foregrounds the real window and clips the cursor to it, starts only from a click on an entry's
content.

## Why

- The shell refocuses its panel whenever be-host comes back to the foreground, so focus mode
  started from the shell's focus trapped the person again each time they returned to be-host.
- It broke dragging panels by their title bars.

## Rejected

- Starting focus mode from the shell's `focus` action: the two problems above.
