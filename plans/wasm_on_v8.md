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
2. **Cold start.** The host tests take twice the CPU they took on precompiled modules
   (decisions/guests_run_on_v8.md). Profile a host test to see what costs (V8's lazy compile of
   each function as it first runs, Liftoff's slower code, tier-up), measure host start to the
   Files pane on its own, and weigh V8's code cache on disk for the libraries against it.
