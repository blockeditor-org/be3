# Anchored popups choose dropdown or sheet by the finger that opened them

Every anchored popup (`MenuPopup`, `Select`, `Popover`, the date and time picker, the colour picker) sits on `unstyled::Anchored`, whose `AnchoredStyle::new(dropdown, sheet)` takes both looks. It samples `touched_by_finger()` when it opens, and shows a sheet if a finger opened it; `Dialog` uses the same rule. `MenuButton`, `PopupTrigger`, `ContextMenu` and the host menu no longer track touch themselves.

## Why

Only `MenuPopup` adapted to touch before, and the "was it touched" tracking was copied in `unstyled::MenuButton` and `unstyled::ContextMenu`. Narrowing to fingers means a pen's barrel button still drops a menu down and a pen opens a dialog as a card.
