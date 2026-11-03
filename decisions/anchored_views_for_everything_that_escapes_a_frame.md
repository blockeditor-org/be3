# Anything that escapes its frame is an anchored view

Context menus and their submenus, popovers, tooltips, the text-selection toolbar, small dialogs opened from a control and sheets attached to an edge are all one mechanism: an anchored view, a screen the app numbers itself (`ScreenId::opened`), lays out against its view's anchor space, paints and commits in the frame that opens it. It replaced the Popup and Overlay window roles, `overlays_in_windows`, dock card dialogs with an anchor, `ShellCommand::Rename` and `ShellCommand::AppMenu`. The picker, rename card and host menu are children placed inside an anchored view of the editor that asked for them.

## Why

- Something that points back to where it came from must not be clipped by the app frame that opened it, and on native it must show with zero frames of delay. The app places and draws it in its own frame commit, so nothing is mounted and there is no round trip; the host only clips it to the bounds and composites it.
- One mechanism instead of several window roles and shell commands: anchored views gain everything screens have (child placements, their own input and accessibility).

## Rejected

- A separate list of layers in the frame commit (`frame-commit.layers`, `layer-id`, `child-placement.in-layer`, `timed-input.layer`), as first designed: it was replaced by making each layer an ordinary screen, so it needs no features of its own.
