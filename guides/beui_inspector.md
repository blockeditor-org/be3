# The beui inspector

The inspector is beui's built-in debugging panel: the node tree, accessibility,
performance, and the Sim tab's input simulation, filters, and screen reader.
[beui.md](beui.md) is the guide to writing beui code; this one is about
inspecting a document that already runs.

## Opening it

Ctrl+Shift+I in a standalone beui window opens the node, accessibility, and
performance inspector. Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves
keyboard focus into the panel and back out again, and Escape inside the panel
returns focus to the document, so the whole inspector is reachable without a
mouse. Its tree rows select and expand together: clicking a row, or pressing
Enter or Space on it, selects the node it lists and opens or closes its
children, and the arrow keys walk the tree.

## Simulating the other input device

The Sim tab converts one input device into the other, so a pointer device can
drive touch behavior and a touchscreen can drive pointer behavior. Both run
inside `Document::show`, so they work the same in a standalone window and in a
beui block editor plugin. The Sim tab also switches the document's theme at
runtime.

"Emulate touch with mouse" turns mouse presses into touch events.

"Simulate mouse with touch" turns the whole shown rectangle into a trackpad and
paints a cursor the document reacts to. One finger moves the cursor, a tap
clicks it, a tap followed by a press and drag drags with the primary button, and
two fingers scroll smoothly. The strip along the bottom holds the left, middle,
and right mouse buttons plus a keyboard toggle: a button stays held for as long
as its finger is down, another finger can work the trackpad at the same time,
and swiping up or down on the middle button scrolls a wheel tick at a time. The
keyboard toggle opens an on-screen keyboard that sends `Event::Key` and
`Event::Text`; its Shift, Ctrl, and Alt keys latch until the next key, and they
also apply to clicks, so Ctrl+Shift+I on it reopens the inspector.

The strip and the keyboard take their room out of the window rather than
covering it: `Document::show` asks the simulation how tall it is, trims that off
the bottom, and lays the document and the inspector panel out in what is left,
the way a phone's keyboard pushes a page up. Nothing is drawn over content that
is still live, so the bars are opaque.

## Filters

The Sim tab's Filters section puts a blur, a contrast reduction, and the
colour vision simulations over the shown rectangle. They are independent of the
screen reader simulation and of each other, and they need no cooperation from
the document: they are a post-processing pass in `Renderer`, so whatever a
document paints - a plugin's beui pane included - is filtered the same way.

`Context::apply_filter` is what turns them on. It records a `Filter` (the region
in points, a blur radius, a contrast multiplier, and a `ColorVision`) and the
number of shapes painted so far, which splits the frame in two: everything
painted before the call goes through the filter, everything painted after it
lands on top of the result untouched. The inspector calls it once a frame,
after the document, the panel, the overlays and the screen reader's focus
outline, and before the reader's readout - so the reader's outline blurs with
the page it marks while the words it is saying stay readable. The region is the
shown rectangle rather than the whole window, so the panel holding the sliders
is never filtered.

The blur is a dual Kawase chain: the frame is halved down a level at a time,
then tented back up, with the number of levels taken from the radius. A radius
of 120 points costs no more than a radius of 8, so the slider can go as wide as
it likes without the frame rate following it. The radius reads like CSS
`blur()`: the light from an edge reaches about three times it. Sampling is
clamped to the filter's region at every level, so nothing outside it bleeds in.

Contrast and the colour vision matrices run in the same pass that composites the
blur. The matrices are the Viénot, Brettel and Mollon linear-RGB
approximations, applied in linear light; the contrast reduction pulls toward mid
grey in gamma space, the way CSS `contrast()` does.

A filter keeps the damage rectangle the rest of the frame is drawn from, so a
filtered frame costs no more to repaint than an unfiltered one. Contrast and
colour vision are per-pixel, so they need nothing beyond the region that
changed; a blur spreads light out of it, so `Prepared::widen` grows the damage
by the chain's reach - the sum of what every pass can move a sample - clipped to
the filter's own region. The scene texture and the blur chain are retained the
way the frame is, and every pass is scissored to the widened rectangle: what
lies outside it was left correct by the frame before, because the reach bounds
what a change can touch at every level. `Renderer::prepare` returns the repaint
it settled on, and it upgrades a partial one to the whole frame when the filter
itself changed - a new radius or a filter switched off restyles everything the
region covers, damage or no damage.

## The screen reader simulation

"Simulate a screen reader" at the bottom of the Sim tab hands the document over
to what a screen reader would say. A readout takes a fixed strip off the bottom
of the shown rectangle, above the mouse simulation's own bar when both are on,
holding the last thing the reader said and how far through the document it is.
It is a strip the document does not get rather than a sheet over it, so the
height stays the same whatever the reader says - a bar that grew with the text
would relay out the page on every utterance - and a long phrase is clipped
rather than wrapped. The highlight marking where the reader is paints over the
document under the filters, so turning the blur or the contrast reduction up is
what leaves the shape of the page with nothing on it to read; the readout is
outside the filtered region and stays legible however far they go.

Nothing about the simulation reads the beui tree. It walks the AccessKit tree
the document publishes every frame, in reading order, and it changes the
document only by sending `ActionRequest`s back through
`Context::accessibility_action` - the same path a platform screen reader uses.
An item is anything with a name of its own, anything that answers Click, Focus,
Increment or `SetValue`, and any scroll area; a control's own text is folded
into its name instead of being read separately, the way a platform computes one.
What gets spoken is the name, the role, the value when it differs from the name,
and then the state - checked, expanded, selected, a slider's percentage,
"dimmed" for a disabled control. A control with nothing to name it is announced
as its bare role, which is the point: "button" on its own is the bug.

While the simulation is on the document answers no pointer or keyboard input
directly, so a click lands nowhere and the only way through the UI is the
simulation. Keyboard and touch drive it at the same time, with no mode to pick
between them. From the keyboard, the left and right (or up and down) arrows walk
an item at a time, Tab and Shift+Tab move between controls, Home and End jump to
the ends, Enter or Space activates, Minus and Plus adjust, Page Up and Page Down
scroll, and R repeats the current item. Walking past either end says so and
reads the item again after it, so the readout never leaves you without the thing
you are standing on. By touch, dragging a finger reads whatever is under it,
flicking left or right moves an item at a time, flicking up or down adjusts a
value, a double tap activates, two fingers tapping repeats, and dragging two
fingers scrolls; turning "Emulate touch with mouse" on as well is what makes
those gestures reachable from a mouse. The same commands sit in the panel as
buttons, so the whole simulation can be driven without either device, and the
Sim tab lists them under those buttons.

Ctrl+Shift+F still parks keyboard focus in the panel, and Escape returns it; the
panel does not otherwise hold focus while the simulation is on, so clicking one
of the command buttons leaves the keyboard commands working.
