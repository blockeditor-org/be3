# Styled components: set aside

Depends on: none.

Set aside until the user picks them up.

## A denser stacked dock in a narrow panel

The inspector's stacked dock spends a lot of height on its regular-sized bar
and padding, and its switcher sheet (`beui-dock-host/src/styled/stack.rs`) opens
half-height with an "Open tabs" heading and "Nothing else is open." over the
rows of its unclosable tools, so in a short window the rows sit below the fold
until the sheet is scrolled or dragged up. A denser stacked bar and switcher
for narrow panels (less tall, less padding) would serve the inspector and any
other dock in a side panel.

## A rounded clip for app dialog regions

A `Below` region is composited under its parent's whole surface, so an app
dialog's square corners would show through the scrim past the card's
rounding. beui has no rounded clip and the region is a GPU drawing, so the
block picker insets the creation `Subregion` by `Space::Sm`
(`CREATION_INSET` in `be-app-workspace/src/picker/view.rs`), enough for
`Radius::Lg`; the app's background matches the card, so the inset doesn't
show. A rounded clip carried through the app protocol is the general fix,
needed if more cards host regions. Keep the protocol free of beui types.
