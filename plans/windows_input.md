# Windows input

Input work left for the desktop shell on Windows. guides/desktop/windows.md
describes what is built: focus mode (the physical mouse and keyboard drive the
real window, the cursor clipped to it, a hotkey back), `SendInput` injection
for XR controller presses (`crates/be-host-win32-input`), and no low-level
hooks.

- **Keeping the shell's pointer and keyboard when a window takes the foreground
  (not planned).** In VR, a real window that comes forward on its own (a dialog,
  a program that grabs focus) takes the keyboard from be-host, and the person
  takes off the headset to give it back (or uses the hotkey, once it can be
  reached). Fixing that needs `WH_MOUSE_LL` and `WH_KEYBOARD_LL` hooks that see
  physical input wherever the foreground is and swallow it while the shell owns
  it, handing it to beui. Constraints if it is ever done:
  - The hooks run on a thread of their own with its own message loop, which must
    answer each event fast: Windows silently removes a hook that exceeds
    `LowLevelHooksTimeout` (about a second; since Windows 7 it unhooks without
    telling), so the callback only decides swallow or pass and posts the event.
  - They don't see input while an elevated window is foreground unless be-host
    runs elevated too (UIPI), so the elevated-window notice would still be
    needed.
  - Injected events come back through the hooks; tell them apart by
    `LLMHF_INJECTED`/`LLKHF_INJECTED` and the `INJECTED` tag.
  - Swallowing the Win key leaves the Start menu to open on release; send an
    unassigned virtual key (0xE8) around it.
- **Switching panels when the cursor leaves a clipped window.** In focus mode
  the clip keeps the cursor inside one entry. Moving the cursor past its edge
  could release the clip and focus the panel it lands on in space, instead of
  needing the hotkey; that needs the panel layout from the shell and the cursor
  position (`EVENT_OBJECT_LOCATIONCHANGE` on `OBJID_CURSOR`, pushed, not polled).
