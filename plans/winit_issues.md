# Issues with winit and android-activity

What beui's native runner (`crates/beui/src/app/native.rs`) works around in
winit 0.30 and android-activity 0.6.1 (with GameActivity 4.4.0), and in the
libraries beside them. Each entry says what is wrong, what we do about it,
and what would let the workaround go. Remove an entry when its workaround is
removed.

## winit 0.30 on Android

- **Drops GameActivity's text input.** `InputEvent::TextEvent` and
  `TextAction` fall into the "unknown event" arm. beui reads
  `AndroidApp::text_input_state()` in `about_to_wait` instead, which a
  GameTextInput change still reaches, and types the difference
  (`app/soft_keyboard.rs`). winit PR 4673 makes those events request a
  redraw; IME events would replace the workaround.
- **Never reports modifiers.** There is no `ModifiersChanged` on Android.
  beui derives Ctrl, Alt and Shift from the modifier keys' own presses
  (`held_modifiers`), and `MainActivity` sends a synthetic modifier press
  around a key a soft keyboard only marked with a meta state.
- **Implicit `set_ime_allowed`.** It calls `show_soft_input(true)`, which
  the system may ignore. beui shows and hides the keyboard itself, explicitly,
  and shows it again when a tap lands in the focused field, since nothing
  reports that the user closed it.
- **No safe area.** winit 0.30 has no inset API (0.31 has
  `Window::safe_area`), and ignores `MainEvent::InsetsChanged`.
  `MainActivity.onApplyWindowInsets` sends the system-bar and display-cutout
  insets to `beui::set_safe_area`, and the runner hands the app the rect
  inside them. The keyboard's inset is not included, so it can still cover
  a field.
- **No back gesture.** winit 0.30 has no back event, predictive or not: with
  `enableOnBackInvokedCallback` the system never sends `KEYCODE_BACK`, and
  without it there is no gesture progress. Each activity registers an
  androidx `OnBackPressedCallback`, which reports the predictive gesture's
  start, progress, cancel and completion to `nativeBack` and on to
  `beui::send_android_back`, and beui enables it through the activity's
  `setBackHandled` only while the document would take back
  (`app/back.rs`). Below Android 13 the Back key still arrives as a key, so
  `dispatchKeyEvent` hands it to the same dispatcher. A back event in winit
  would move the forwarding out of Java, but the enabling would stay, since
  it decides whether the system plays its own animation.

## android-activity 0.6.1 and GameActivity

- **Null text before any input.** GameTextInput's state starts with a null
  text pointer, which `text_input_state()` passes to
  `slice::from_raw_parts`; with debug assertions that aborts.
  `MainActivity.onCreate` gives it an empty state.
- **64 KiB text buffer.** GameTextInput copies at most 64 KiB but reports the
  full length, so a longer text is read past the buffer. The soft keyboard
  clears its text once it passes 1 KiB, and a single paste over 64 KiB would
  still overflow.
- **Asynchronous `set_text_input_state`.** The change is applied later on the
  UI thread, so a read right after it can return the old text. The clear
  above waits until the text no longer starts with what it cleared.
- **GameTextInput takes keys.** While the soft keyboard is up, its key
  listener puts every key with a character, and the arrows, into its own
  buffer. `MainActivity.dispatchKeyEvent` sends Tab, Escape, the arrows,
  Home, End, Page Up and Down, the modifier keys and any key with Ctrl, Alt
  or Meta straight to the native side.
- **Build.** GameActivity's Java half extends AppCompat, so the APK carries
  the AppCompat closure (`buck/android/BUCK`, `maven_artifacts`), and its C
  half is compiled by a fixup, since buck does not run android-activity's
  build script (`third-party/rust/fixups/android-activity`).

## accesskit_android 0.7.5

- **Overflow on an empty text.** The text-changed event computes
  `len() - 1` and relies on it wrapping for an empty string; with overflow
  checks it panics on the UI thread and aborts. The crate is built with
  `-Coverflow-checks=off` (`third-party/rust/fixups/accesskit_android`).
  Still present in 0.9.0.
