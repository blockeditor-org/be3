# Terminal screen rows overflow visibly rather than with an ellipsis

In beui-terminal-view, screen rows joined by `wrapped` are one `Text` with
`Overflow::Visible` and a `<Break/>` at each wrap point.

## Rejected

- `Overflow::Ellipsis`: it would cut each row on its own and draw an ellipsis
  the terminal's cells do not hold whenever the view is briefly narrower than
  the grid.
