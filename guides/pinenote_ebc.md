# The PineNote's e-paper driver

hrdl's `rockchip_ebc` drives the PineNote's panel (guides/pinenote.md). It is not upstream, and
sr.ht does not answer web fetches from here, but `git clone` works:

- https://git.sr.ht/~hrdl/pinenote-dist: the distribution (packages, configs, mkosi image).
  `packages/linux-pinenote-hrdl-git/PKGBUILD` names the kernel branch it builds.
- https://git.sr.ht/~hrdl/linux, branch `v6.19_pinenote` at the time of writing: the kernel.
  The driver is `drivers/gpu/drm/rockchip/rockchip_ebc.c`, its scheduling in
  `rockchip_ebc_blit_neon.c`, its ioctls in `include/uapi/drm/rockchip_ebc_drm.h` (below). Clone
  only those with `git clone --depth 1 --filter=blob:none --sparse -b v6.19_pinenote
  https://git.sr.ht/~hrdl/linux` and `git sparse-checkout set drivers/gpu/drm/rockchip
  include/uapi/drm`.
- https://git.sr.ht/~phantomas/pinenote-service: the daemon behind pinenote-dist's per-window
  hints in sway, which beui does not use.

## How it draws

The driver keeps a hint per pixel (`hints_ioctl`) and reads it when it converts a committed
framebuffer, only inside the commit's `FB_DAMAGE_CLIPS`. Changing a hint therefore changes only
how later changes are drawn; pixels no commit touches stay as they are. The `RECT_HINTS` ioctl
sets the default hint for the whole screen (`set_default_hint`) and then any rectangles.

A hint's bits 4–5 pick the bit depth and with it the waveform: Y1 is DU (black and white), Y2 is
DU4 (four greys), Y4 is GL16 (sixteen greys). Bit 6 dithers instead of thresholding, which
matters for Y1 and Y2. A transition runs to its end before the pixel can change again, except
that a DU transition between black and white can be cut short and reversed
(`early_cancellation_addition`), so only Y1 keeps up with motion.

Bit 7, REDRAW, first draws the pixel as dithered Y1 with DU, then, once it has not changed for
`redraw_delay` hardware frames (about 85 a second), redraws it at its own depth. The driver
ignores the bit while `redraw_delay` is 0, the kernel's default; pinenote-dist's
`/etc/modprobe.d/rockchip_ebc.conf` sets `default_hint=0xa0 redraw_delay=200`.

`GLOBAL_REFRESH` redraws the whole screen with GC16, the flashing waveform that clears ghosts.
`MODE`'s `DRIVER_MODE_FAST` makes every update a cancellable black and white one, but it is
global, waits for ongoing updates, and enters through a global refresh, so beui does not use it.

## What beui-platform-drm does

`epaper.rs` sets the default hint to Y2 threshold (`0x10`) while the screen is still, and to
dithered Y1 (`0x40`) while it moves (guides/pinenote.md says when). To settle it sets Y2 again
and commits the unchanged framebuffer with damage clips covering everything drawn at Y1, which
the driver converts again and redraws at Y2. It leaves `redraw_delay` alone and never sets
REDRAW.

## `include/uapi/drm/rockchip_ebc_drm.h`

```c
#ifndef __ROCKCHIP_EBC_DRM_H__
#define __ROCKCHIP_EBC_DRM_H__

#include "drm.h"
#include "drm/drm_mode.h"

#if defined(__cplusplus)
extern "C" {
#endif

#define ROCKCHIP_EBC_HINT_BIT_DEPTH_Y1 0 << 4
#define ROCKCHIP_EBC_HINT_BIT_DEPTH_Y2 1 << 4
#define ROCKCHIP_EBC_HINT_BIT_DEPTH_Y4 2 << 4
#define ROCKCHIP_EBC_HINT_BIT_DEPTH_MASK 3 << 4
#define ROCKCHIP_EBC_HINT_THRESHOLD 0 << 6
#define ROCKCHIP_EBC_HINT_DITHER 1 << 6
#define ROCKCHIP_EBC_HINT_REDRAW 1 << 7
#define ROCKCHIP_EBC_HINT_MASK 0xf0

#define ROCKCHIP_EBC_DRIVER_MODE_NORMAL			0
#define ROCKCHIP_EBC_DRIVER_MODE_FAST			1
#define ROCKCHIP_EBC_DRIVER_MODE_ZERO_WAVEFORM		8
#define ROCKCHIP_EBC_DRIVER_MODE_PHASE_SEQUENCE		9

#define ROCKCHIP_EBC_DITHER_MODE_BAYER		0
#define ROCKCHIP_EBC_DITHER_MODE_BLUE_NOISE_16	1
#define ROCKCHIP_EBC_DITHER_MODE_BLUE_NOISE_32	2

struct drm_rockchip_ebc_trigger_global_refresh {
	bool trigger_global_refresh;
};

/**
 * struct drm_rockchip_ebc_off_screen - Pointer to userspace buffer.
 * @info1: unused.
 * @ptr_screen_content: pointer to width * height * buffer containing
 *   the horizontally flipped off screen. The highest four bits of each
 *   byte are ignored.
 */
struct drm_rockchip_ebc_off_screen {
	__u64 info1;
	__u64 ptr_screen_content;
};

/**
 * struct drm_rockchip_ebc_extract_fbs - Pointers to userspace buffers. Pointers
   that are NULL are skipped without warning.
 * @ptr_packed_inner_outer_nextprev: pointer to 3 * width * height buffer
 *   containing packed tuples of inner counters, outer counters, and a
 *   packed Y4 (next, prev) pairs.
 * @ptr_hints: pointer to width * height buffer for hints.
 * @ptr_prelim_target: pointer to width * height buffer containing packed Y4
     (prelim, target) pairs.
 * @ptr_phase1: pointer to width * height / 4 buffer containing the first of
     the double-buffered phase buffers.
 * @ptr_phase2: pointer to width * height / 4 buffer containing the second of
     the double-buffered phase buffers.
 */
struct drm_rockchip_ebc_extract_fbs {
	char *ptr_packed_inner_outer_nextprev;
	char *ptr_hints;
	char *ptr_prelim_target;
	char *ptr_phase1;
	char *ptr_phase2;
};

/**
 * struct drm_rockchip_ebc_rect_hint - Pixels hint definition
 * @hints: Pixel hints for all pixel in the rectangle.
 * @padding: Padding for 64bit alignment.
 * @rect: Rectangular region on which the hints should be applied.
 */
struct drm_rockchip_ebc_rect_hint {
	__u8		hints;
	__u8		padding[7];
	struct drm_mode_rect	rect;
};

/**
  * struct drm_mode_rect_hints
  * @set_default_hint: Reset hints to their default before applying new ones.
  * @default_hint: Hint to use for uncovered pixels.
  * @padding: 64bit alignment padding.
  * @num_rects: Number of rectangles contained in rect_hints
  * @rect_hints: Pointer to an array of struct drm_rockchip_ebc_rect_hint.
  */
struct drm_rockchip_ebc_rect_hints {
	__u8	set_default_hint;
	__u8	default_hint;
	__u8	padding[2];
	__u32	num_rects;
	__u64	rect_hints;
};

/**
 * struct drm_rockchip_ebc_mode - Query and set driver/dither  modes and
 *   redraw delay
 * @set_driver_mode: apply driver_mode instead of reading it to the same
 *   field.
 * @driver_mode: one of ROCKCHIP_EBC_DRIVER_MODE_NORMAL or
 *   ROCKCHIP_EBC_DRIVER_MODE_FAST.
 * @set_dither_mode: apply dither_mode instead of reading it to the same
 *   field.
 * @dither_mode: one of ROCKCHIP_EBC_DITHER_MODE_BAYER,
 *   ROCKCHIP_EBC_DITHER_MODE_BLUE_NOISE_16, or
 *   ROCKCHIP_EBC_DITHER_MODE_BLUE_NOISE_32.
 * @redraw_delay: number of hardware frames to delay redraws.
 * @set_redraw_delay: apply redraw_delay instead of reading it to the same
 *  field.
 */
struct drm_rockchip_ebc_mode {
	__u8	set_driver_mode;
	__u8	driver_mode;
	__u8	set_dither_mode;
	__u8	dither_mode;
	__u16	redraw_delay;
	__u8	set_redraw_delay;
	__u8	_pad;
};

/**
 * struct drm_rockchip_ebc_zero_waveform- Query and enable/disable zero
 *   waveform mode
 * @set_zero_waveform_mode: apply zero_waveform_mode instead of reading it to
 *   the same field.
 * @zero_waveform_mode: 0 for disable(d), 1 for enable(d)
 */
struct drm_rockchip_ebc_zero_waveform {
	__u8	set_zero_waveform_mode;
	__u8	zero_waveform_mode;
	__u8	_pad[6];
};

#define ROCKCHIP_EBC_MAX_PHASE_SEQUENCES 96
#define ROCKCHIP_EBC_MAX_REGIONS 8

struct drm_rockchip_ebc_phase_sequence_element {
	__u32			delay_ms;
	__u8			num_frames;
	__u8			_pad[3];
	__u32			num_regions;
	struct drm_mode_rect	rect[ROCKCHIP_EBC_MAX_REGIONS];
	__u8			phase[ROCKCHIP_EBC_MAX_REGIONS];
};

struct drm_rockchip_ebc_phase_sequence {
	__u8						num_seqs;
	__u8						do_init;
	__u8						do_gc16;
	__u8						gc_target;
	__u8						do_force_temperature;
	__u8						force_temperature;
	__u8						_pad[6];
	__u32						delay_ms;
	struct drm_rockchip_ebc_phase_sequence_element elms[ROCKCHIP_EBC_MAX_PHASE_SEQUENCES];
};

#define DRM_ROCKCHIP_EBC_NUM_IOCTLS		0x07

#define DRM_IOCTL_ROCKCHIP_EBC_GLOBAL_REFRESH	DRM_IOWR(DRM_COMMAND_BASE + 0x00, struct drm_rockchip_ebc_trigger_global_refresh)
#define DRM_IOCTL_ROCKCHIP_EBC_OFF_SCREEN	DRM_IOW(DRM_COMMAND_BASE + 0x01, struct drm_rockchip_ebc_off_screen)
#define DRM_IOCTL_ROCKCHIP_EBC_EXTRACT_FBS	DRM_IOWR(DRM_COMMAND_BASE + 0x02, struct drm_rockchip_ebc_extract_fbs)
#define DRM_IOCTL_ROCKCHIP_EBC_RECT_HINTS	DRM_IOW(DRM_COMMAND_BASE + 0x03, struct drm_rockchip_ebc_rect_hints)
#define DRM_IOCTL_ROCKCHIP_EBC_MODE		DRM_IOWR(DRM_COMMAND_BASE + 0x04, struct drm_rockchip_ebc_mode)
#define DRM_IOCTL_ROCKCHIP_EBC_ZERO_WAVEFORM	DRM_IOWR(DRM_COMMAND_BASE + 0x05, struct drm_rockchip_ebc_zero_waveform)
#define DRM_IOCTL_ROCKCHIP_EBC_PHASE_SEQUENCE	DRM_IOW(DRM_COMMAND_BASE + 0x06, struct drm_rockchip_ebc_phase_sequence)

#if defined(__cplusplus)
}
#endif

#endif /* __ROCKCHIP_EBC_DRM_H__*/
```
