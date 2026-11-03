# Text has one `overflow` prop, defaulting to wrapping at words

`Overflow` (`Wrap { at: WrapAt, max_lines }`, `Ellipsis`, `Visible`, in `beui_core::font`) replaced `wrap`, `clip` and `ellipsis` on the base text node, `primitives::Text` and `styled::Text`. It defaults to `Wrap { at: Words, max_lines: None }`. Former `clip=true` uses became `Ellipsis`; there is no `Clip` mode. The terminal's screen rows stay `Visible`.

## Why

Text used to overflow its container by default, and ellipsis was ignored when wrapping. The `clip=true` uses were titles or a value, where text cut off mid-letter reads as the whole thing; an ellipsis says something is missing. Text that genuinely needs clipping is `Visible` inside something that clips. `Ellipsis` cuts each `Break`-separated row on its own, but for the terminal it would draw an ellipsis the terminal's cells don't hold while the view is narrower than the grid.
