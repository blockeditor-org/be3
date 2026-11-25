# On Android, C++ beside V8 links the NDK's shared libc++

On Android be-host links V8 (third-party/v8), which brings its own libc++ inside
librusty_v8.a, and Oboe (cpal's audio backend, oboe-sys), compiled against the NDK. Oboe links
the NDK's shared libc++ (`libc++_shared.so`, cpal's `oboe-shared-stdcxx` in be-host-runtime),
which the APK ships beside libbe_host.so (buck/android/defs.bzl). Any other C++ a crate builds
for Android does the same: cc-rs's default there.

## Why

V8's libc++ keeps most of the library in its own ABI namespace (`std::__Cr`), but the classes
the standard leaves unversioned, the `std::logic_error` and `std::runtime_error` family among
them, are plain `std::` in every libc++. The NDK's static libc++ (`libc++_static.a`) defines
them too, so a link with both fails on duplicate symbols. The shared one resolves them at run
time and does not collide.

Until November 2026 HarfBuzz, compiled by cc-rs, put `-lc++_shared` on the link and hid oboe-sys's
default `-lc++_static`; replacing FreeType and HarfBuzz with Rust crates removed it and be-host
stopped linking for Android.

## Rejected

- Oboe on the static libc++, with V8's copies of the unversioned classes hidden: that means
  rebuilding V8 (scripts/internal/build-rusty-v8-android.sh) for a link flag, and two copies of
  the classes exceptions are thrown as.
- V8 built against the NDK's libc++ (`use_custom_libcxx=false`): Chromium supports it poorly
  and it changes the V8 every guest runs on for one audio library.
