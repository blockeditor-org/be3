# On Windows the shell's `focus` window action only gives the surface beui's keyboard focus, never focus mode

On the Windows host, the shell's `focus` window action (a press on a panel's title bar, the
picker, a tab given the keyboard: `WindowSurface`'s `focus`) does nothing but give the surface's node
beui's keyboard focus (`win32_content::CapturedContent`). Focus mode, which
foregrounds the real window and clips the cursor to it, starts only from a click on an entry's
content.

## Why

- The shell refocuses its panel whenever be-host comes back to the foreground, so focus mode
  started from the shell's focus trapped the person again each time they returned to be-host.
- It broke dragging panels by their title bars.

## Rejected

- Starting focus mode from the shell's `focus` action: the two problems above.
