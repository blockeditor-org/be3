# Driving an app with `drive`

Every beui program can run headless and be driven from the shell, which is the way to see a
change working in the real app:

    ./scripts/buck run //crates/block-app:dev     # or //crates/beui-demo:dev, //crates/be-launcher:dev
    source ~/.cache/be3/dev/env                   # beui-demo: ~/.cache/be3/beui-demo/env
    drive tree

`:dev` builds the program, starts it with no window and returns once it has settled; run it
again to pick up a rebuild. block-app's starts signed in, with a workspace called Dev open.
After `--`, `--fresh` deletes its data first and `--stop` stops it. Its output goes to
`~/.cache/be3/NAME/app.log`.

More:
- guides/running/drive/launching.md: launcher options, driving any beui program, two at once,
  `be-drive run` for tests, and what a headless run does not exercise.
- guides/running/drive/reading.md: the tree's format, targets, `hit` and `state`.
- guides/running/drive/acting.md: input commands in detail, settling and waiting, the clock,
  scripts.
- guides/running/drive/window.md: resizing, focus, clipboard, file dialogs, dragging files in,
  closing.

## Commands

A TARGET is `#TEST_ID`, `X,Y` (or `X,Y,WIDTH,HEIGHT`) in the tree's pixels, or text found in
one line of the tree, such as `'Button "Create"'`. `drive help` prints this list too.

    Reading
      tree                          the accessibility tree, one node per line
      ids                           every test id on screen, with where it is
      find TARGET                   where TARGET is
      hit TARGET                    what a press there would reach, without pressing
      state                         window, cursor, pointer, fingers, keys, input method, dialogs
      actions [all]                 what the command palette would offer (all: every action)
      shot FILE [TARGET]            a PNG of the window, or of TARGET
    Pointer
      click TARGET [right|middle|double]
      move TARGET                   hover
      down TARGET [BUTTON] / up [TARGET] [BUTTON]
      drag FROM TO [STEPS]
      wheel TARGET TICKS [XTICKS]   mouse-wheel ticks, down when positive
      scroll TARGET DY [DX]         a touchpad scroll in pixels
      zoom TARGET FACTOR            a touchpad pinch
      leave                         the pointer leaves the window
    Touch
      tap TARGET [FINGER] / swipe FROM TO [FINGER] / pinch TARGET FACTOR
      touch down|move|up [FINGER=]TARGET...; touch up lifts every finger
      back / back [left|right] / back PROGRESS / back commit|cancel
    Keyboard
      type TEXT                     a newline presses Enter
      key CHORD...                  such as ctrl+z Enter shift+Tab
      keydown KEY... / keyup KEY...
      ime compose TEXT / ime commit [TEXT] / ime cancel
      act ACTION_ID [PANE]          run an action as the command palette does
    Screen reader
      a11y TARGET [ACTION [VALUE]]  click, focus, increment, set VALUE, expand, ...
    Files
      grab FILE... / drop [TARGET] / ungrab
      upload FILE [TARGET] / dismiss
    Window (headless only, except focus, blur and close)
      resize WIDTHxHEIGHT[@SCALE] / resize screen WIDTHxHEIGHT
      focus / blur / clipboard [TEXT] / close
    Waiting
      settle / wait TEXT|#TEST_ID / gone TEXT|#TEST_ID
      pause MILLISECONDS            by the app's clock
      clock stop / clock run
    Options, before the command
      --changes=N|all  --no-settle  --timeout=SECONDS  --socket=PATH
      drive - < FILE                one command a line, stopping at the first that fails

Every input command waits until the app has settled and prints what changed in the tree as
`-` and `+` lines, so there is no need to wait or read the whole tree again.
