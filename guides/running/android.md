# Running be-host on Android

The Android emulator needs KVM. Check before anything else:

    test -r /dev/kvm && test -w /dev/kvm && echo ok

Without KVM the emulator's system image never becomes usable, even booted without
acceleration, so if that does not print `ok`, installing the SDK or working around it is wasted
time: only building the APK works, and the handoff message should say the change was not run on
Android. Claude Code's cloud containers have no KVM: `/dev/kvm` does not exist and the CPU does
not expose `vmx`/`svm` to the guest.

## APK identities

The package id in the APK manifest determines the installed app and its private
storage. These variants install side by side:

| Target | Package id | Launcher label |
| --- | --- | --- |
| `:android` (local) | `com.be3.block.dev` | Block (Dev) |
| `:android-dist` (CI) | `com.be3.block.ci` | Block (CI) |
| `:android-release` (deploy) | `com.be3.block` | Block |

Deploy can override its id and label with `--android-id` and `--android-label`.
The Java activity remains `com.be3.block.MainActivity` in all variants; it is
independent of the installed package id. The bundle's JSON `program_id` does not
select the Android package or storage directory.

## Starting it

    ./scripts/buck run //crates/be-host:android
    ./scripts/android-emulator

The first command builds the APK into `target/android/be-host.apk`. The second boots the
emulator if it is not running, installs the APK if it changed, launches be-host, waits
`--wait` seconds (20 by default) and prints its log and whether it is still running.
It exits non-zero if be-host died. It writes `logcat.txt`, `app.log` and `screen.png` to
`/tmp/be3-android`. `--apk PATH` installs a different APK, which is how to compare a
branch against main. The runner defaults to the dev package; for a CI or
deployed APK also pass `--package com.be3.block.ci` or `--package com.be3.block`.
`./scripts/android-emulator stop` shuts the emulator down.

The APK only carries arm64 code, and the emulator is x86_64, so be-host runs under
Android's arm64 translation. A native crash's backtrace stops at the translation layer
and does not show be-host's frames. Its stdout and stderr, where Rust's panic
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
accessibility tree be-host gives Android, with each node's class, text and bounds on
screen, which is how to find what to tap.

Tap a text field before typing into it. The keyboard takes the bottom of the screen, and
be-host's area shrinks to what is left.

## TalkBack

    adb shell settings put secure enabled_accessibility_services com.google.android.marvin.talkback/com.google.android.marvin.talkback.TalkBackService
    adb shell settings put secure enabled_accessibility_services '""'    # off

The first time it is turned on, TalkBack opens its own tutorial and permission prompts
over be-host; relaunch it with `adb shell am start -n com.be3.block.dev/com.be3.block.MainActivity`.

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

## Signing the APK

Both APKs require Android 7 (API 24), matching the native toolchain, target SDK 35, and are
signed with APK Signature Scheme v2 alone (decisions/android_apks_signed_with_v2_only.md). Buck
builds only aligned, unsigned templates without build metadata; `buck build --out` gives one of
those, which cannot be installed. `buck run` adds `assets/build-info.json` and signs on this
machine with `be-tool-local apk-sign` (its doc comment has the key sources and flags), writing
`target/android/be-host.apk` or `target/android/be-launcher.apk`. The build helper
(guides/build_server.md) and `./scripts/deploy` sign through the same tool, so no key is ever an
action input.

Android accepts an update only when it is signed with the same key as the installed app, so
losing a key means its installs can never be updated; replacing one means uninstalling first.
