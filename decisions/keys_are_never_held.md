# Keys are never held

No host holds keys, anywhere, and nothing waits on an editor to claim a key or the keyboard. A key
goes to whatever has the keyboard at the moment it is routed. If that is a child whose app does
not run yet, the key is dropped, as is every press and move an app that is not running would
hear; it is not queued for when the app starts. Only where the keyboard is (`Input::Focus`),
state rather than input, reaches the app once it runs.

Moving the keyboard into a child is declarative: the child's placement asks for it
(`child-placement.keyboard-ask`, the serial of the editor's latest ask, which grows with each
ask). At the layout commit that carries a serial above every one it acted on for the editor,
the host moves the keyboard to the view that shows the child if it is up, or else out of the
placer's own view at once, the child's view taking it once it is up; keys typed in between
reach nothing. It still does so only while the keyboard is inside the placer's views and
no popover or keyboard-taking layer is open, which it checks synchronously at that commit;
otherwise the ask does nothing. An editor that makes a block to type into ("Add notes") places
its child asking once it places it; a `ChildBlock` or `Subregion` in a dock tab asks each time
the dock gives the tab the keyboard.

The ask ends without moving anything when the keyboard is outside the placer's views and the
children they show, or a keyboard-taking view is open, when the commit carries it. A waiting
ask also ends when its child is no longer placed, its app fails, or the editor's app restarts
(the new run counts its asks from 1 again).

No queue is left. Input for a screen the host asked a running app for but has not announced
to it yet is dropped (be-host-runtime's `Instances::gate`), as for an app that is not running
(`forward_view`, which keeps only `Input::Focus`): the user decided ("do nothing
special") that a screen not announced yet gets nothing special, so input made in the frame
before a view is announced is lost: keys, presses and modifier changes alike (the modifiers it
held are forgotten, so the next input that gets through sends them, and the screen's pressed
buttons are forgotten when it is announced). Where the keyboard is stays state: when the host announces
a screen that has the keyboard, it tells the app so (`Instances::announce`), which is also how
a restarted app's focused view hears it has the keyboard. Keys typed right after "Add notes",
before the text app has the notes' screen, reach nothing; typing once the notes are up does.

Under a root, an Escape the active child does not handle does not take the keyboard back to
its placer either: only a press outside the child or an Escape in the placer's own view takes
the child back (be-sdk-root's `Children::revoke`, plans/host_as_platform_adapter.md), and
the user decided it stays so.

## Why

A slow editor must never slow down how the app feels. Holding keys made the keyboard's
behaviour depend on how fast an editor started, laid out and answered: keys typed into one
place could surface seconds later somewhere else, and every part of the host that routes input
had to know whether some editor still owed or was owed keys. One rule (a key goes where the
keyboard is) is predictable, and the cost of a dropped key while an editor starts is smaller
than the cost of the whole app waiting on it.

An ask carried in the placement is state, not an event: the host reads it with the commit that
places the child, so it cannot arrive before the child exists or miss a child the editor has
since replaced, which a call had to be answered for (`gift-missed`).

## Rejected

- **Held keys with timers** (be-host-runtime's `gift`, removed): an editor's `focus-child` or
  `focus-block-child` started a gift that held the keys typed into the editor's view until the
  child's view took the keyboard and its app was ready, bounded by `GIFT_WAIT` (3 s) on the
  frame clock or `GIFT_COMMITS` (8) of the editor's layout commits counted from the child being
  up; keys typed right after a press waited for the frame that answered it (`GiftTo::Answer`).
  It needed a stage machine, a held-keys queue per view, keys owed between views as content
  moved, and a miss answer, and still made what a person typed depend on how slow an editor
  was.
- **A queue for screens not announced yet** (be-host-runtime's `Instances::unannounced_input`,
  `take_announced_input`, removed): it kept the input for a screen the app had not been told
  of and handed it over once it was, so keys typed while a child's editor opened in a running
  app reached it. It made that case unlike an app that is not running, and the user chose to
  drop the input instead.
- **A `bool` in the placement** ("should have the keyboard"): it cannot say that the dock gave
  the same tab the keyboard again, and two children asking in one commit cannot say which asked
  last. The serial says both.
