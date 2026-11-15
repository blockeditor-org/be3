# Running be-host on Android

Until third-party/v8/BUCK pins a V8 for Android, nothing that runs guests builds for Android (plans/wasm_on_v8.md).

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
| `:android` (local) | `com.be3.host.dev` | be-host (Dev) |
| `:android-dist` (CI) | `com.be3.host.ci` | be-host (CI) |
| `:android-release` (deploy) | `com.be3.host` | be-host |

Deploy can override its id and label with `--android-id` and `--android-label`.
The Java activity remains `com.be3.host.MainActivity` in all variants; it is
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
deployed APK also pass `--package com.be3.host.ci` or `--package com.be3.host`.
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
over be-host; relaunch it with `adb shell am start -n com.be3.host.dev/com.be3.host.MainActivity`.

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

## Push notifications

Through a UnifiedPush distributor the person installs (decisions/android_push_via_unifiedpush.md;
the Java is `crates/be-host/android/java/com/be3/host/`, the Rust `crates/be-host/src/android_push.rs`).

- *The switch.* The settings list's "Notifications" row (`be_host_base::push_device`, installed
  by `android_push::install`) reads `MainActivity.pushState`: unsupported with no distributor
  (one whose receiver takes `org.unifiedpush.android.distributor.REGISTER`, listed under
  `<queries>` in the manifest), denied once the person refused `POST_NOTIFICATIONS` (Android
  13+). Turning it on asks for that permission, then sends `REGISTER` with a token kept in the
  app's preferences and the server's VAPID key, to the distributor chosen before, or the only
  one installed; with several and none chosen it asks in a plain Android chooser and keeps the
  answer. Turning it off sends `UNREGISTER`, forgets the endpoint (`device_push::turn_off`), so
  nothing registers it again, and has the server forget it for the open organization.
- *Registering.* The distributor answers `NEW_ENDPOINT` to `PushReceiver`, which keeps the
  endpoint in the host state beside this device's RFC 8291 keys (`push_receiver`, made once,
  `be_host_stack::device_push::keys`); the host registers it as `PushService::WebPush` for each
  organization it opens once connected, and again after a new endpoint (`android_push::sync`).
- *Showing.* `MESSAGE` carries the raw encrypted body. `PushReceiver` may run with no activity:
  it loads `libbe_host` itself and, on a thread while the broadcast is held (`goAsync`), calls
  `nativeMessage`, which runs `device_push::receive` on the files directory: decrypt, drop a push
  already shown (`shown_pushes`, by a hash of the sealed bytes, for
  `MAX_NOTIFICATION_AGE_MILLISECONDS`; one that does not decrypt is dropped too, as a browser
  drops it), then open it through the stack when it runs in this
  process in that organization, or else through a `Peer::cached` built from the refs database
  (`refs_in`, the handle the process shares, so an activity starting meanwhile uses the same one)
  for each saved account that kept that organization, naming the sender from the contact names
  and the block from the summaries kept there. A push that decrypts but that nothing here
  can open or name shows "New message" naming nothing. It shows on the "Messages" channel, tagged by block, and acknowledges the message
  (`MESSAGE_ACK`).
- *Pressing* starts `MainActivity` (or brings it back, `onNewIntent`) through the activity-alias
  `com.be3.host.OpenNotification`, with the organization and block, which `nativeOpen` holds
  for the host to open (`pressed_notifications`), as on the web. The alias is not exported and
  `MainActivity` takes a block only from an intent naming it, since `MainActivity` itself is
  exported for the launcher and any app could send it extras.
- *Backups.* `allowBackup="false"`, and `res/xml/data_extraction_rules.xml` excludes every
  domain from cloud backup and device transfer (Android 12+, where `allowBackup` no longer
  covers transfers): the host state holds identity secrets and push keys, and the refs
  database every block's keys.
- *Signing out* the last saved account sends `UNREGISTER`; every sign-out names the endpoint to
  `logout`, so the server forgets it (`android_push::sign_out`).

Nothing of this has run on a device or the emulator yet; plans/notifications.md lists what to
check there, including that
`adb shell am start -n com.be3.host.dev/com.be3.host.MainActivity --es com.be3.host.organization ORG --es com.be3.host.block BLOCK`
opens nothing.

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
