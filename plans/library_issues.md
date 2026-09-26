# Issues with the libraries beui's runners use

What beui's runners work around in the libraries they are built on. Each
entry says what is wrong, what we do about it, and what would let the
workaround go. Remove an entry when its workaround is removed.

## accesskit_android 0.7.5

- **Overflow on an empty text.** The text-changed event computes
  `len() - 1` and relies on it wrapping for an empty string; with overflow
  checks it panics on the UI thread and aborts. The crate is built with
  `-Coverflow-checks=off` (`third-party/rust/fixups.bzl`).
  Still present in 0.9.0.
