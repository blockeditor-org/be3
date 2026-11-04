# The PineNote's e-paper driver

hrdl's `rockchip_ebc` drives the PineNote's panel (guides/pinenote.md). It is not upstream, and
sr.ht does not answer web fetches from here, but `git clone` works:

- https://git.sr.ht/~hrdl/pinenote-dist: the distribution (packages, configs, mkosi image).
  `packages/linux-pinenote-hrdl-git/PKGBUILD` names the kernel branch it builds.
- https://git.sr.ht/~hrdl/linux, branch `v6.19_pinenote` at the time of writing: the driver is
  `drivers/gpu/drm/rockchip/rockchip_ebc.c`, its scheduling `rockchip_ebc_blit_neon.c`, its ioctls
  `include/uapi/drm/rockchip_ebc_drm.h`. Clone only those:

      git clone --depth 1 --filter=blob:none --sparse -b v6.19_pinenote https://git.sr.ht/~hrdl/linux
      git -C linux sparse-checkout set drivers/gpu/drm/rockchip include/uapi/drm

- https://git.sr.ht/~phantomas/pinenote-service: the daemon behind pinenote-dist's per-window
  hints in sway, which beui does not use.

## How it draws

The driver keeps a hint per pixel and reads it only when it converts a commit's
`FB_DAMAGE_CLIPS`. Changing a hint changes only how later changes are drawn. The `RECT_HINTS`
ioctl sets the default hint for the whole screen and then any rectangles.

| Hint bits | Meaning |
| --- | --- |
| 4–5 | depth and waveform: Y1 = DU (black and white), Y2 = DU4 (four greys), Y4 = GL16 (sixteen greys) |
| 6 | dither instead of threshold |
| 7 | REDRAW: draw as dithered Y1 first, then at its own depth once unchanged for `redraw_delay` hardware frames (about 85 a second) |

A transition runs to its end before the pixel can change again, except a DU transition between
black and white, which can be reversed partway; only Y1 keeps up with motion. REDRAW is ignored
while `redraw_delay` is 0 (the kernel default); pinenote-dist's
`/etc/modprobe.d/rockchip_ebc.conf` sets `default_hint=0xa0 redraw_delay=200`.

`GLOBAL_REFRESH` redraws everything with GC16, the flashing waveform that clears ghosts. `MODE`'s
`DRIVER_MODE_FAST` is global, waits for ongoing updates and starts with a global refresh, so beui
does not use it.

beui-platform-drm (`epaper.rs`) uses only `RECT_HINTS` with no rectangles (`0x10` still, `0x40`
moving) and `GLOBAL_REFRESH`; it leaves `redraw_delay` alone and never sets REDRAW.
