# The dock keeps its panels with KeepAlive

Depends on: none. (The format-bar work on keyboard gifts into joined editors, `content_moved` and
`key_placers` in be-host, does not touch the dock's hand-off; where a dock tab view takes the focus
it clears the instance's main-view rebuild, `forget_main_rebuild` in be-host's `app_view.rs`, which
matters only if kept panels rebuild app view catchers.)

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
1. The keyboard hand-off, in the dock's keyboard bookkeeping (`unstyled/keyboard.rs`,
   `remember_focus` and the `content_keyboard` path `content_focused` feeds): a `content_focused`
   for a tab the dock hid since (no longer its pane's active entry) is recorded as the focus to
   give back when that tab is shown again, not taken as a request to show it. An app refocusing
   its trigger as a popup closes is normal and must not move the dock; the host cannot tell a
   stale report from a real click, so the fix is not there. Check it against what
   `content_focused` showing a hidden tab is for (a stacked dock handing on the keyboard a tab's
   content had: `a_stacked_dock_hands_on_the_keyboard_a_tabs_content_had_until_something_else_takes_it`).
2. App children (`be-sdk-beui` `child.rs`, `SubregionView`) follow `kept_shown()` and report
   hidden as it turns false, before their scope pauses, rather than through `measure`.

**Tests.** The three host tests above; a dock tab hidden behind another is paused and kept (its
effects wait and run once shown, nothing rebuilt); nested-dock tests in beui-host.
