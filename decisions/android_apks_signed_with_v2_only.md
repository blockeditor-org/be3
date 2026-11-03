# Android APKs are signed with APK Signature Scheme v2 alone

Every be-host and be-launcher APK (local, CI and release) requires Android 7 (API 24) and is signed with APK Signature Scheme v2 only, by `be-tool-local apk-sign`, which writes the v2 signing block itself. See guides/running/android.md, Signing the APK.

## Why

- The minimum version, API 24, matches the native toolchain, and v2 covers every version from API 24.

## Rejected

- Adding a v3 signature: v3 only adds key rotation.
