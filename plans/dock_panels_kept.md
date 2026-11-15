# The dock keeps its panels with KeepAlive

Depends on: the format-bar work on keyboard gifts into joined editors (`content_moved`,
`key_placers` in beui-dock-host) landing first; this builds on its hand-off.

**Today.** beui-dock-host keeps every panel it built in hand-written caches: `panels` and
`panel()`/`keep_panels`/`forget_panel` in `unstyled.rs`, and `BarPanels`/`bar_panel()`/
`keep_bar_panels` in `unstyled/bars.rs`. Each panel lives in a scope under the dock's owner and is
shown through a `Portal`; a hidden panel keeps running.

**Goal.** One `KeepAlive` in the dock, keyed by an enum of tab and bar panels, with `keep` listing
what the dock still holds. Each place that shows a panel shows a `Kept` (a `Dynamic` over the shown
tab around a `Kept` for the pane, the fullscreen view and the back preview; a `Kept` per bar for
the bar slots, the shared bar and the lifted bar). A hidden panel pauses. The dock's lookups
(`panel_node`, `bar_panel_rect`, dialog anchors) read a signal of what is built, filled and emptied
by the `Kept` content.

**What a first attempt found.** It passed the beui-host dock tests but broke three be-host host
tests (`the_host_menu_opens_under_its_button`,
`the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside`,
`a_popup_in_space_closes_on_a_press_in_another_panel`): a press on a workspace tab that closes an
app's popup in another tab no longer shows that tab.

- **Late refocus.** The closing popup puts the app's focus back on its trigger. That arrives as
  `content_focused` for the old tab after the press showed the new one, and shows the old tab
  again. The old code was immune only by timing: the new panel was built synchronously while
  detached, so `focus_into` on it went pending and then cleared the focus, which took the keyboard
  from the app. With a `Kept` the panel is built by the view's control flow, so the keyboard is
  given once it is built, which can be after the app's refocus.
- **Stale measure reports.** Bindings in a paused panel do not run, so what a panel reports from
  `measure` (an app child's placement) stays at its last value while hidden.
- **Joined docks stay awake.** The panel of a tab whose own dock joined this one is replaced in the
  layout by that dock's group and never shown, yet the inner dock's effects must keep running.
  `KeepAlive` needs an `awake` key set for it (keys whose hidden subtree keeps running and counts
  as shown).
- **The dock's own `kept_shown`.** A panel hidden behind another tab, or behind the stage its
  nested dock joined, must not read as hidden to `DockJoin`, or the join lets go and nested docks
  loop; with `awake` counting as shown, only the holder tabs need it.

**Prerequisites.**
1. The keyboard hand-off: the tab shown takes the keyboard (from an app child too) before an app's
   late refocus can reach it, whether or not its panel is built yet.
2. App children (`be-sdk-beui` `child.rs`, `SubregionView`) follow `kept_shown()` and report
   hidden as it turns false, before their scope pauses, rather than through `measure`.

**Tests.** The three host tests above; a dock tab hidden behind another is paused and kept (its
effects wait and run once shown, nothing rebuilt); nested-dock tests in beui-host.
