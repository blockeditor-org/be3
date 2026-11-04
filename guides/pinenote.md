# The PineNote

be-host runs as the desktop in a login session (`BEUI_PLATFORM=drm be-host --desktop`,
guides/desktop/login_session.md) on a Pine64 PineNote with hrdl's Arch Linux
(https://git.sr.ht/~hrdl/pinenote-dist): hrdl's kernel with its `rockchip_ebc` e-paper driver
(guides/pinenote_ebc.md), greetd with gtkgreet as the login screen.

## The display and the GPU are two devices

The panel's KMS device (`rockchip-ebc`, connector `DPI-1`, 1872×1404) cannot draw: it reads each
framebuffer on the CPU (linear XRGB8888, RGB565 or R8) and turns it into waveforms. The Mali-G52
draws, through panfrost's render node and Mesa's panvk.

beui-platform-drm handles any such pair: when its Vulkan device's render node is not the display
device (`buffers` in `runner.rs`), it allocates buffers with GBM on the render node and imports
them into the display device by PRIME (`swapchain.rs`, `Imported`). simpledrm and evdi displays
take the same path. Every frame carries `FB_DAMAGE_CLIPS`, so the driver reads only what changed;
a rotated screen (guides/desktop/login_session.md, Rotation and pointing devices) is turned on the
GPU, clips included, so the driver always sees the panel's own
orientation.

## E-paper

`epaper.rs` in beui-platform-drm finds the panel by driver name and switches the driver's default
hint: four greys while the screen is still, dithered black and white while anything animates or
a finger, pen or button is held, then settles back with a frame that redraws what was drawn fast.
Its doc comments say when and why. pinenote-dist's per-window hints (pinenote-service and its
sway bridge) are not used.

Fast waveforms leave ghosts. be-host publishes the `Epaper` host value when the platform has an
`Epaper` and turns `EpaperRequest::Refresh` into a global refresh (`display/linux.rs`); the
desktop app then shows a refresh button on the Status bar and a tile in Quick settings.

## Input

The touchscreen (`cyttsp5`) and the stylus are ordinary libinput devices; the stylus becomes
`Event::Pen`. Their calibration matrices (pinenote-dist's udev rule) must match the unturned
panel: the platform turns touch, pen and tilt by the screen's rotation itself, so a rotating
matrix would turn them twice. There are no letter keys, so the on-screen keyboard opens by
itself when a field is tapped. Holding four fingers for three seconds ends the session
(`escape.rs`), in place of Ctrl+Alt+Backspace.

## Reading books

Books open in KOReader, a Wayland program, in the tab of the book's file block
(guides/desktop/linux.md, Files opened in programs). It needs a KOReader whose `.desktop` entry
lists `application/epub+zip` in `MimeType` (the AppImage needs one written for it), and
`shared-mime-info` for the `.epub` glob.

## Building and installing

    ./scripts/buck build -m release //crates/be-host:native --target-platforms root//buck/platforms:linux_arm64 --out DIR

Copy DIR to the PineNote and run `DIR/be-host --install-login-session` there; it also lists the
session in `/etc/greetd/environments` for gtkgreet's menu. On first sign-in be-host finds the
e-paper panel and makes the device's settings profile an E-paper tablet (E-paper theme, instant
motion, the rotate button; guides/blocks/workspaces.md, The account's Settings block and device
profiles). The panel reports 210 × 157 mm on a `DPI` connector, which gives a
scale of 1.5 (guides/desktop/linux.md, Displays).
