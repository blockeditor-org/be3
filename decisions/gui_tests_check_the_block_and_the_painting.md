# GUI tests check the block and the painting, and find widgets by test id

A GUI test asserts on two things: the content the editor's operations reached, and what the editor painted, as a paint snapshot. It finds widgets by a test id (`@test_id`), keyed by what the block keys them by. This applies to editor tests, DocumentTest, the host tests and crates/beui-tests' scripts. See guides/testing_a_gui.md, 1 and 2.

## Why

- An editor's job is to turn gestures into operations, so the block is what says it worked.
- The painting catches what an assertion on the block cannot: a control that vanished, a panel that lost its contents, a colour that changed.
- A test id survives renaming a button, giving it an icon or moving it to a sidebar.

## Rejected

- Asserting on coordinates, widget sizes or the order of the accessibility tree: they change whenever anyone touches a layout and say nothing about whether the editor works. Text metrics also differ between beui's native and HTML cores.
- Finding widgets by label: renaming or restyling a control would break its tests.
- Keying repeated widgets by the order they are drawn in: the order is not what the block or its edits name.
