# V8 on Android, and the last measurements

Depends on: none.

Every wasm guest runs on V8 and nothing is precompiled (decisions/guests_run_on_v8.md). What is
left:

1. **Android's V8.** rusty_v8 publishes no Android build, so until third-party/v8/BUCK pins one,
   nothing that runs guests builds for Android: the APK, the build helper's `android` product, and
   a deploy that keeps its `android` input on. `scripts/internal/build-rusty-v8-android.sh`
   builds the two files on a person's machine (`librusty_v8_release_aarch64-linux-android.a.gz`
   and `src_binding_release_aarch64-linux-android.rs`, for V8 152.2.0) and prints each one's
   sha256 and size. Once they are hosted, `android_arm64` and `android_arm64_binding` in
   third-party/v8/BUCK name them; then remove android_arm64 from `_UNBUILDABLE` in
   buck/dev/broad.bxl and the notes in guides/running/android.md and guides/build_server.md, and
   run the APK on a device.
2. **A thread's own instances.** Every guest thread runs on an isolate of its own, so each
   thread V8 starts instantiates its app and every library it links again: the module is
   compiled once and shared (`Compiled`), but instantiating and linking the dylink imports repeat
   per thread, and code V8 optimises for one isolate is shared through the module while each
   isolate's own wrappers and feedback are not. Measure what a thread's start costs in a host test (an app
   that spawns threads, such as the text editor), and if it shows, make a thread's instance
   cheaper: link against the trimmed export list (`keep_exports`) once per module rather than
   per instance, or keep a warm isolate per guest to hand a new thread. Done: a thread's start
   measured, and either made cheaper with the before and after in decisions/guests_run_on_v8.md
   or recorded there as not worth it.
