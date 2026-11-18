# V8: Android checked

Depends on: none.

Every wasm guest runs on V8 and nothing is precompiled (decisions/guests_run_on_v8.md). What is
left:

1. **Android's V8, checked.** third-party/v8/BUCK pins the Android files the user built with the
   fixes now in scripts/internal/build-rusty-v8-android.sh made by hand. Done: a clean run of
   the script on a fresh machine makes files with the pinned sha256s (or new pins from it), and
   the APK opens the dev workspace on a device.
2. **A thread's own instances: dropped.** Each guest thread runs on an isolate of its own and
   instantiates its app and every library it links again. plans/single_threaded_guests.md step 3
   leaves guests with no threads and its step 4 deletes `thread-spawn`, so there is no thread
   start left to make cheaper; don't measure or optimise it before then. A worker
   (be_sdk::worker) is one instance of its own module started once per app, not per thread.
