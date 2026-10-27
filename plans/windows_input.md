# Windows input

How the desktop shell on Windows (guides/desktop/windows.md) gets the person's
mouse and keyboard into the Windows windows and monitors it shows as panels,
and what is left for later.

## Where it stands

- **Focus mode is the main path.** Choosing a Windows entry (from the picker,
  a click in its panel, a controller) raises the real window, makes it the
  foreground window, moves the cursor into it and clips the cursor to its frame
  bounds, or to its monitor for a display entry (`ClipCursor`, applied again on
  foreground and location changes). The physical mouse and keyboard then drive
  it natively; block-app sees none of that input. A `RegisterHotKey` hotkey
  returns focus to block-app and releases the clip (be-win32). Elevated windows
  work in this mode: UIPI blocks only injected input.
- **Injection is for XR controller clicks.** `crates/be-win32-input` turns an
  event at a point in an entry's panel into `SendInput` entries (absolute
  virtual-desktop coordinates, keys by scan code, text by `KEYEVENTF_UNICODE`,
  each tagged `INJECTED` in `dwExtraInfo`), raises and foregrounds the target
  window first, and refuses with a typed `InjectError` when the window's
  process runs at a higher integrity level (`SendInput` doesn't report that
  failure) or the input desktop is the secure one, so the host can show a notice.
- **No low-level hooks.** Nothing calls `SetWindowsHookEx` or
  `RegisterRawInputDevices`, and nothing swallows physical input.

## Deferred

- **Keeping the shell's pointer and keyboard when a window takes the foreground
  (not planned).** In VR, a real window that comes forward on its own (a dialog,
  a program that grabs focus) takes the keyboard from block-app, and the person
  takes off the headset to give it back (or uses the hotkey, once it can be
  reached). Fixing that needs `WH_MOUSE_LL` and `WH_KEYBOARD_LL` hooks that see
  physical input wherever the foreground is and swallow it while the shell owns
  it, handing it to beui. Constraints if it is ever done:
  - The hooks run on a thread of their own with its own message loop, which must
    answer each event fast: Windows silently removes a hook that exceeds
    `LowLevelHooksTimeout` (about a second; since Windows 7 it unhooks without
    telling), so the callback only decides swallow or pass and posts the event.
  - They don't see input while an elevated window is foreground unless block-app
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
