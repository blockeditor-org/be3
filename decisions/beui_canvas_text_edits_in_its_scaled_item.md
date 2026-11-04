# Canvas text is edited in its scaled item, with no font-size or scale prop

Canvas text is a `styled::TextArea` in its `scales=true` canvas item both while edited and while read-only (`find=false`, `disabled` while not edited, `size=TextSize::Custom`, `color=TextColor::Custom`); the item only takes presses while edited. Scaled spaces take pointer input through their scale, so zoom comes from the scaled space and the text area has no font-size or scale prop.

## Why

A scaled item used to take no pointer input, so edited text moved into an ordinary item and faked the zoom with `font_size = size × zoom`. That could wrap differently from the read-only text, laid the text out again on every zoom while editing, and needed a font-size prop nothing else wanted. It was also the last import of `unstyled::` outside beui.
