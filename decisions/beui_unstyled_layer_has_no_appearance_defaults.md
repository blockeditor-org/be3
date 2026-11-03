# The unstyled beui layer has no appearance defaults

Colours, sizes, paddings, breakpoints and placeholder renderers are required props on `beui-components-unstyled` components and are chosen by the styled layer. Behaviour defaults stay: the drag and row-drag thresholds, zoom limits, a slider's 0..1 range, tooltip and toast durations, and the dock's starting sidebar and floating window sizes and minimum pane sizes. Defaults that draw nothing also stay: zero spacings, thumbs, radii, insets and fades, `ScrollbarStyle::default()` (no bar), a picture's white tint, pass-through panels, and the dock's optional pieces. Under this rule `TextInputStyle::default()`, `TextAreaColors::DEFAULT`, `SyntaxColors::DEFAULT`, `MenuStyle::default()`, the text area's default renderers and geometry (now a required `TextAreaLook`), `SHEET_STOPS` (moved to styled), scrims, `Selectable`'s colours, `Scroll`'s focus ring, `Choice`'s spacing and the dock's splitter and sidebar widths became required or moved to styled.

## Why

Someone using only the unstyled layer can then implement any style without inheriting ours. Behaviour defaults are what someone uses unstyled for instead of building their own, so they are kept. The text area keeps its list, quote, margin and heading ratios as proportions of the given `font_size`, so a zoomed canvas text keeps its proportions.
