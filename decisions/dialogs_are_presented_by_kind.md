# One Dialog, presented by its kind and size

There is one `Dialog`, and how it is presented follows from what it is:

1. Small per-object forms opened from a control (text settings, file-tree Inspect, a database filter, canvas Export, Rename): a card with a beak on desktop and a bottom sheet on phones. They escape the frame, have no scrim, close on an outside press, block presses on the owning frame, trap focus and give it back.
2. Confirmations: centred over the frame that asked, with a scrim, inside its region. Never a sheet.
3. Large forms and embedded editors (block creation, Share, Members, Contacts, conflict compare): attached to the owning frame, not a point; a full-height sheet on phones. The anchor only drives the opening transition.
4. Host-level modals with no origin (lock, reauth, onboarding, add device, sync errors): drawn by the host, centred over the host window.
5. Program dialogs (Wayland and X): windows centred on their parent.
6. The command palette: a top-centred overlay.

A small dialog points at the control that opened it; if that was a menu row that has closed, at what the menu was opened on; if the command came from a menu another view drew, a shortcut or the palette, at the editor's tab (`use_frame_anchor`); and if none is visible, at a zero-height rect at the top centre of the owning frame, where it draws no beak.

## Why

- A small dialog is always one placement anchored to a rect, so no second way of hanging it from a frame is needed.
- The beak is drawn only when the anchor is something the user can see.

## Rejected

- `HangFrom` and `DialogAnchor::Frame`: replaced by the anchor fallback above.
