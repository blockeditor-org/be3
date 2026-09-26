# Running the app on Android

The Android emulator needs KVM. Check before anything else:

    test -r /dev/kvm && test -w /dev/kvm && echo ok

If that does not print `ok`, give up on the emulator: do not install the SDK, boot the
emulator without acceleration, or try to work around it. Without KVM the system image
never becomes usable. Say in your handoff message that the change was not run on Android.

## Starting it

    ./scripts/buck run //crates/block-app:android
    ./scripts/android-emulator

The first command builds the APK into `target/android/block-app.apk`. The second boots the
emulator if it is not running, installs the APK if it changed, launches the app, waits
`--wait` seconds (20 by default) and prints the app's log and whether it is still running.
It exits non-zero if the app died. It writes `logcat.txt`, `app.log` and `screen.png` to
`/tmp/be3-android`. `--apk PATH` installs a different APK, which is how to compare a
branch against main. `./scripts/android-emulator stop` shuts the emulator down.

The APK only carries arm64 code, and the emulator is x86_64, so the app runs under
Android's arm64 translation. A native crash's backtrace stops at the translation layer
and does not show the app's frames. The app's stdout and stderr, where Rust's panic
messages go, are in logcat under the tag `RustStdoutStderr`, and are the way to find out
why it aborted.

## Driving it

`adb` is `~/Android/Sdk/platform-tools/adb`. The screen is 1080x2400 physical pixels, and
every coordinate below is in them.

    adb exec-out screencap -p > screen.png
    adb shell input tap 540 1200
    adb shell input text 'hello'
    adb shell input keyevent 67        # backspace; 4 is back, 3 is home

`adb shell uiautomator dump /sdcard/ui.xml && adb shell cat /sdcard/ui.xml` is the
accessibility tree the app gives Android, with each node's class, text and bounds on
screen, which is how to find what to tap.

Tap a text field before typing into it. The keyboard takes the bottom of the screen, and
the app's area shrinks to what is left.

## TalkBack

    adb shell settings put secure enabled_accessibility_services com.google.android.marvin.talkback/com.google.android.marvin.talkback.TalkBackService
    adb shell settings put secure enabled_accessibility_services '""'    # off

The first time it is turned on, TalkBack opens its own tutorial and permission prompts
over the app; relaunch the app with `adb shell am start -n com.be3.block/.MainActivity`.

`input tap` is injected past TalkBack's touch exploration, so under TalkBack it still
clicks. To touch the screen the way a finger does, write to the touchscreen's device with
`sendevent`. Its axes run from 0 to 32767, so scale x by 32767/1080 and y by 32767/2400:

    adb shell "sendevent /dev/input/event2 3 57 7; sendevent /dev/input/event2 3 53 $X; \
      sendevent /dev/input/event2 3 54 $Y; sendevent /dev/input/event2 3 58 50; \
      sendevent /dev/input/event2 0 0 0; sendevent /dev/input/event2 3 57 -1; \
      sendevent /dev/input/event2 0 0 0"

`adb shell getevent -pl` lists the devices; the touchscreen is `virtio_input_multi_touch_1`.
One touch moves TalkBack's focus to the node under it, and two in a row within one
`adb shell` are a double tap, which activates the focused node. The double tap's timing
through `adb` is not reliable, so run it a few times before concluding it is broken, and
compare against main with `--apk`.
