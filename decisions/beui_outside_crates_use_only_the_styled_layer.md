# Code outside beui depends only on the styled layer

Crates outside beui's own crates import nothing from `unstyled::` or `primitives::`, and do not write `Extent::px` (be-tool-fix-rust-source enforces both). Styled re-exports the behaviour-only pieces that carry no look (`BackSlide`, `Draggable`, `DropTarget`, `Floating`, `Pressable`, `MenuItem`, `TreeItem`, `TextAreaState`, `PanZoom`, ...), and `dock::styled` re-exports the dock's model types. Left on purpose: logic-grid's hotbar slot face and deterministic-game's board (app canvases with their own drawing), and beui-demo's two custom button faces on `unstyled::Button`, kept as samples of building a face on the unstyled layer.

## Why

A crate that needs a face of its own is a sign of a missing styled component. Auditing each import produced styled components such as `SwatchButton`, `TileButton`, `ToolPill`, `Banner`, and `MenuButton`'s `leading` slot, replacing private faces (database's `ControlFace`, be-sdk-beui's `BottomDock` and infinite-canvas's `ZoomPill`).
