# The PineNote

block-app runs as the desktop (`--session`, guides/linux_desktop.md) on a Pine64 PineNote with
hrdl's Arch Linux (https://git.sr.ht/~hrdl/pinenote-dist): hrdl's kernel with its `rockchip_ebc`
e-paper driver, greetd with gtkgreet as the login screen, and sway as the session it ships.

## The display and the GPU are two devices

The panel is driven by `rockchip-ebc`, a KMS device
(`/dev/dri/by-path/platform-fdec0000.ebc-card`, connector `DPI-1`, 1872×1404) that cannot draw:
it reads each framebuffer on the CPU (XRGB8888, RGB565 or R8, linear only, through
`drm_gem_shmem`) and turns it into waveforms itself. The Mali-G52 draws, through panfrost's
render node and Mesa's Vulkan driver, panvk.

beui-adapter-drm handles any such pair: when the Vulkan device it opened has a render node that
is not the display device's own (`buffers` in `runner.rs`), it allocates each output's buffers
with GBM on that render node and imports them into the display device by PRIME
(`swapchain.rs`'s `Imported`, which is what wlroots does for sway here); otherwise it uses
smithay's `GbmBufferedSurface` on the display device. simpledrm and evdi displays take the same
path. Each frame carries `FB_DAMAGE_CLIPS` (both paths): the renderer's scissors for a partial
repaint plus where the pointer was and is (`Screen::draw`), so the driver reads only what
changed; a full repaint sends none, which means the whole plane.

## The e-paper panel

`epaper.rs` in beui-adapter-drm finds the panel by the display device's driver name. The
driver draws each pixel by its hint: a bit depth (Y1, Y2 or Y4), threshold or dither, and
whether to redraw it later with a slower, cleaner waveform. The adapter sets the default hint
to Y2, threshold, no redraw (`0x10`: four greys on the fast waveform) when it starts and
whenever it takes the seat back, and puts back the hint it found (the module's
`default_hint`) when it ends. pinenote-dist's per-window hints (pinenote-service and its sway
bridge) are not used.

Fast waveforms leave ghosts. The adapter provides `beui_adapter_drm::Epaper`, whose `refresh()`
runs the driver's global refresh, a full redraw that clears them. block-app publishes the
`Epaper` host value (`true`) when the adapter provides one and turns each `EpaperRequest::Refresh`
into a refresh (`display/linux.rs`); linux-desktop shows a refresh button (`desktop.refresh`)
beside the keyboard button while the value is true.

## Input

The touchscreen (`cyttsp5`) is an ordinary libinput touch device; pinenote-dist's udev rule
gives it the calibration matrix that matches the panel. The stylus is a libinput tablet tool,
which the adapter does not read yet. No input device has letter keys, so the on-screen keyboard
opens by itself when a field is tapped. With no keyboard for Ctrl+Alt+Backspace, holding four
fingers on the screen for three seconds ends the session (`escape.rs`), which works as long as
the adapter's event loop runs. The power button is logind's, which powers off by default.

## Building and installing

Build the app with its plugins for arm64 here, copy the directory to the PineNote, and install
it there as a session:

    ./scripts/buck build -m release //crates/block-app:native --target-platforms root//buck/platforms:linux_arm64 --out DIR
    DIR/block-app --install-session

`--install-session` also lists the session in `/etc/greetd/environments`, which gtkgreet offers
on its session menu. The app links GTK 3 and WebKitGTK (`webkit2gtk-4.1`), which pinenote-dist
does not install. `BEUI_SCALE=2` suits the panel's 227 dpi.

`devices/pinenote/vulkan-panfrost-26.3` is an optional PKGBUILD that builds panvk alone from
Mesa 26.3 and replaces the `vulkan-panfrost` package. It exposes Vulkan 1.3 on the G52;
Mesa 26.2 exposes Vulkan 1.0. Build it on the PineNote with `makepkg -si`. The build overrides
arch-meson's forced automatic features so unrelated graphics frontends stay optional.
panvk refuses the G52 (Bifrost, v7) unless
`PAN_I_WANT_A_BROKEN_VULKAN_DRIVER` is set, so the package also installs
`/etc/profile.d/panvk.sh`, which greetd reads before it starts a session. Mesa 26.3.0-rc1 still
reports conformance 0.0.0.0 for the G52, so also set
`WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=1` when launching block-app; every renderer, the
session's included, reads wgpu's instance environment settings. Allowing the adapter does not
guarantee correct rendering with either Mesa version. Move `_tag` to the
release once Mesa 26.3.0 is out, and drop the package once Arch's `vulkan-panfrost` is 26.3 or
newer.
