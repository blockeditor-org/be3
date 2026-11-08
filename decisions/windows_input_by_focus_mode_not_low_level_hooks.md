# The Windows desktop shell gives windows real input through focus mode, not low-level hooks

On Windows, the person's physical mouse and keyboard reach a Windows window or
monitor shown as a panel by focus mode: be-host raises and foregrounds the real
window, moves the cursor into it and clips the cursor to it, and a registered
hotkey brings focus back. be-host sees none of that input. Only XR controller
presses, which have no physical cursor, are injected with `SendInput`. Nothing
installs `WH_MOUSE_LL`/`WH_KEYBOARD_LL` hooks or raw input, and nothing
swallows physical input.

## Why

- Elevated windows work in focus mode: UIPI blocks only injected input.
- Hooks would bring the constraints plans/deferred/windows_input.md lists: a thread of
  their own that must answer every event fast (Windows silently removes a hook
  that exceeds `LowLevelHooksTimeout`), and no input while an elevated window
  is foreground unless be-host runs elevated too.

## Rejected

- Low-level hooks that swallow physical input while the shell owns it, so a
  window that takes the foreground on its own can't take the keyboard from
  be-host in VR: not planned.
