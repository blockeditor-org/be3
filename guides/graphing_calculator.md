# Build a graphing calculator

The calculator in [be-app-graphing-calculator](../crates/be-app-graphing-calculator) is a small editor for explicit functions of `x`. It uses plaintext equations such as `sin(x)`, `y = x^2` and `1/x`. The same guest is staged on every host platform.

## Declare the document

Follow [Adding an app](adding_an_app.md) to register the editor and its type module. Define the document as a `List<Equation>`, with a sequence-backed `be_block_model::Text` field on each equation. Give each equation its list object's identity. Use that identity as the UI row key and as the operand of a reorder edit.

The definition in [be-type-graphing-calculator](../crates/be-type-graphing-calculator/src/lib.rs) enables the host's undo support. Sharing, file organization, revision history and offline merge require zero feature-specific app code. Structural controls mark undo boundaries with `break_step`. The text editor already groups typing.

## Bind the editor

Import `be-lib-graphing-calculator`, a Rust crate with no BE3 dependencies. Call `Expression::parse` for each equation. Use `Expression::segments` for the visible horizontal range and paint those segments over a grid. The crate handles expression precedence, evaluation and discontinuity sampling.

Bind a `TextArea` to each equation using `LiveText` and `FieldOf::plain(id, Equation::EXPRESSION)`. Pump the binding when the text area changes or the document projection advances. Each row adopts its own text sequence because the projection's operation log has one consumer. Sequence positions keep peer selections attached to their characters through concurrent edits. Publish a `SharedCaret` tagged with the focused equation ID and pass matching peer carets to `remote_carets`. The row remains mounted when its keyed equation moves.

Render the list with `Reorder` and its keyed `ForEach`. On drop, call the document list's `move_into` helper with the moved object's ID and the preceding object's ID. The component does the interaction and animation.

Declare the equation pane and the bare graph stage in one `Docking`. The dock provides the splitter and gathers the equation pane at narrow sizes. Declare the graph node with `editor.content`, enable pan and zoom in the manifest, and use the camera from `editor.canvas()` to draw the scene. Send the zoom and fit controls to the host. Convert a hovered pointer to graph coordinates before publishing it, so a peer can draw it through their own camera.

## Compare the integration

| Feature | Calculator integration | Comparison |
| --- | --- | --- |
| Sharing, file operations, revision history, offline merge | Zero feature-specific app code | Yjs supplies shared types and offline persistence primitives. Document sharing and the file and history UI still need a host. |
| Undo and redo | Mark structural edit boundaries. The text area supplies typing groups. | Yjs has an undo manager and capture boundaries. The SDK already exposes block undo and redo in the frame. |
| Live editing and presence | One text binding per row, sequence positions for caret presence, world coordinates for graph pointers. | Yjs supplies shared text and awareness. An editor binding and the app's presence data are still needed. The per-row `LiveText` pump is a place to reduce integration work. |
| Pan and zoom | Declare the viewport, consume the camera, and wire controls. | D3 Zoom also supplies gesture handling and a transform. The BE3 host also routes gestures between the embedded editor, graph, and sidebar. |
| Animated reorder | Key the rows and commit the drop as a document move. | dnd-kit provides sortable interaction and transitions. Both integrations need to persist the final order. |
| Docking and responsive layout | Declare one layout and use the shared compact behavior. | Compare with a docking library on desktop and a narrow touch screen, including a resize while the keyboard is open. |

The comparison describes integration work from the source. It needs a runtime pass to check behavior. Put two users in the same equation field, move that equation while both are typing, then repeat with one user editing offline. Reconnect and undo the move while preserving both edits. Check sharing, rename, move and revision restoration through the host. Repeat graph navigation and editing on desktop, web, and a phone with the keyboard open.
