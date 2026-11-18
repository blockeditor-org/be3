# V8: Android checked

Depends on: none.

Every wasm guest runs on V8 and nothing is precompiled (decisions/guests_run_on_v8.md). What is
left, deferred until someone with a device and a fresh machine runs it:

1. **Android's V8, checked.** third-party/v8/BUCK pins the Android files the user built with the
   fixes now in scripts/internal/build-rusty-v8-android.sh made by hand. Done: a clean run of
   the script on a fresh machine makes files with the pinned sha256s (or new pins from it), and
   the APK opens the dev workspace on a device.
