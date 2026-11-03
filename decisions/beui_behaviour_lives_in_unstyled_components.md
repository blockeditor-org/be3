# Behaviour lives in unstyled components; styled components only choose the look

Focus, keys, pointer, touch, timers, animation driven by state and accessibility nodes belong to an unstyled component; the styled component chooses fills, tokens and layout from its handle. This is why `unstyled::Modal`, `Dialog`/`DialogBody`, `Toasts`, `Progress` (indeterminate sweep), `Table`, `ColorInput`, `ActionBar`, `Banner`, tree row menus (`TreeRowMenuButton`) and the text input's clear and copy callbacks exist, and why `unstyled::Button`/`Toggle` decide when an icon-only button's tooltip shows (the handle's `tooltip` is empty otherwise) while the styled face draws the `Tooltip` bubble.

## Why

An audit found styled components building their own behaviour, the same logic repeated in several faces (the icon-only tooltip rule was in `MenuButton`, `Popover`, `ToggleButton` and `IconButton`), and pieces other crates could not reach (`DialogSurface` was `pub(crate)`, so be-host could not place a dialog in a given rectangle). With the behaviour unstyled, any beui user can build their own face on it.
