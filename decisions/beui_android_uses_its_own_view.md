# beui on Android runs in its own view, not NativeActivity

beui's Android runner (`crates/beui-platform-android`) has its own activity, `com.be3.beui.BeuiActivity`, whose `BeuiView` is the surface, takes touches, keys and the soft keyboard's input connection, and hosts AccessKit. The Rust side is entered through `android_main`, which beui calls on a thread of its own.

## Why

Input has to reach the program through the Android view system: TalkBack's touch exploration arrives as hover events on a view, and NativeActivity's input queue never delivers them, so a screen reader could not explore the UI.

## Rejected

- NativeActivity and its input queue: it never delivers the hover events TalkBack's touch exploration sends.
