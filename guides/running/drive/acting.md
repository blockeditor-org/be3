# Giving input with `drive`

## Examples

    drive down TARGET; drive pause 600; drive move TARGET; drive up   any gesture, step by step
    drive wheel TARGET 3                      three mouse-wheel ticks down; a second number turns sideways
    drive pinch TARGET 2                      two fingers spreading apart
    drive touch down 0=TARGET 1=TARGET; drive touch move 0=... 1=...; drive touch up
    drive back 0.4                            hold the back gesture part way; back commit or back cancel finish it
    drive key ctrl+z Enter shift+Tab          chords, each pressed and let go in turn
    drive keydown alt; drive key tab tab; drive keyup alt
    drive ime compose ni; drive ime commit 你

`touch` names each finger (`0=TARGET`, `1=TARGET`) so several move in one frame, which is how
to make a pinch of your own; `touch up` with no finger lifts them all. `keydown` on a key that
is already down repeats it.

`a11y TARGET` with no action prints the node a screen reader would act on and the actions it
offers; acting goes through the accessibility tree rather than the pointer or keys, the path a
screen reader takes, and reaches plugins' nodes too.

`type` sends text, which a Wayland program's window does not take; `key` and `keydown` also
send each key's scan code, which it does.

## Settling and waiting

Every command that gives input waits until the app has settled and prints what changed in
the tree, `-` and `+` lines. A long change is cut to its first 60 lines; `drive --changes=N
COMMAND` (or `all`) prints more. The app has settled once a frame asks for no other and
`App::busy` is false; block-app is busy while a plugin is starting, owes a frame or holds
unanswered input, while an account or workspace request is pending, and while the block store
has commands to carry out or is still loading the workspace's graph.

`drive --no-settle COMMAND` answers after the frame that took the input instead, to see the app
part way through something, such as a plugin still loading. Work the app cannot see, such as a
server push, is waited for with `drive wait TEXT` or `drive gone TEXT` (or `#TEST_ID`), which
return once a line of the tree contains TEXT or none does; `drive settle` waits on its own, and
`drive pause MILLISECONDS` lets time pass by the app's own clock. Each command gives up after
30 seconds, or `drive --timeout=SECONDS`.

## The clock

`drive clock stop` stops the app's clock: time then passes only with `drive pause`, a frame at
a time, so an animation can be stopped at any point and a screenshot of it taken the same way
every run, and an app that animates forever still settles. `drive clock run` starts it again.
Anything else waiting on time, such as a plugin's frame timeout, waits for `pause` too.

## Scripts

`drive -` reads commands from standard input, one a line, quoted as a shell would, prints each
before its answer and stops at the first that fails:

    drive - <<'EOF'
    click "Add a root block"
    click 'Label value="Checklist"'
    wait 'Pane "Checklist"'
    shot checklist.png 'Pane "Checklist"'
    EOF
