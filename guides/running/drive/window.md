# The headless window

A headless program's window is simulated, and drive can do to it what a person does to a
real one:

- `drive resize 390x844@3` resizes it and sets its scale, which is how to see a phone's
  layout; `drive resize screen 1920x1080` sets the screen it fills when the app asks for
  fullscreen. Only a headless window can be resized this way.
- `drive focus` and `drive blur` give it focus or take it away.
- `drive clipboard` prints the window's clipboard and `drive clipboard TEXT` sets it; the
  app's own copy and paste use it, and `drive key ctrl+v` pastes it. In a headless window
  the clipboard is the window's own; in a real one it is the system's.
- A file dialog the app opens waits for drive, in any window while drive is attached:
  `drive upload FILE TARGET` clicks TARGET and answers the dialog it opens with FILE
  (without TARGET it answers one already open), and `drive dismiss` cancels it.
- `drive grab FILE...` drags files in from outside, over the pointer; `drive move` carries
  them, to try what hovering with them shows, `drive drop [TARGET]` drops them and
  `drive ungrab` takes them away again.
- `drive close` asks the window to close, as its close button does, in a window too. The app
  then exits, or stays open, to ask about unsaved changes, and answers with what changed.
