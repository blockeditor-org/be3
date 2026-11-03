# Tabs and segmented controls collapse into a Select when their row does not fit

`Tabs` (which absorbed `ResponsiveTabs`) and `styled::SegmentedControl` collapse into a Select when their row doesn't fit the width they are given, using the base node `FirstFit`, which lays out the first child whose natural width fits and leaves the rest unplaced (no paint, input or Tab stop). A segmented control is a separate component over `Choice`'s `Radio` kind: tabs switch a panel, a segmented control picks a value (calendar's Day/Week/Month, be-launcher's Open/Closed, be-host's invite role).

## Why

A collapse that always happens matches how a menu always adapts to touch. Before, the value pickers used `Tabs` and reported tab roles with no tab panel; the segmented control gives them radio semantics.

## Rejected

- A fixed breakpoint: it would have collapsed the inspector's and the date picker's tabs, which sit in narrow panels with room to spare.
