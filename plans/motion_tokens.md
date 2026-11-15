# Motion tokens

Depends on: none.

Styled components take design tokens for space, radii, text and colour (`beui-components-styled`
`tokens.rs`), but there is none for motion, so unstyled components hard-code their curves.
A curve (an `Easing`, a `Spring`) is how a motion looks, so it is appearance: under
decisions/beui_unstyled_layer_has_no_appearance_defaults.md it comes from a token and is a
required prop on the unstyled component. How long a motion takes, and the distances a gesture
works by, are behaviour, like the drag threshold: they stay defaults in unstyled.

The curves this moves:

- `sheet.rs`: the `OutCubic` easing of its enter and return;
- `back_slide.rs`: the easings of its leave and return;
- `rubber_band.rs`: `SCROLL_SPRING`, `WINDOW_SPRING`;
- `beui-dock-host`: `bars.rs`'s `SETTLE` spring, and the easings its `GAP_SECONDS` and
  `unstyled.rs`'s `SIDEBAR_SECONDS` run on.

`glide.rs`'s `GLIDE_SPRING` moves with plans/glide_on_a_signal.md, which uses `Settle` from step 1
here. The durations (`ENTER_SECONDS`, `LEAVE_SECONDS`, `RETURN_SECONDS`, `HANDOFF_SECONDS`,
`GAP_SECONDS`, `SIDEBAR_SECONDS`) stay where they are, as defaults.

## Shape

- **Unstyled** components take each curve as a required prop, a `Spring` or an `Easing`
  (`beui-types` `animation.rs`). Durations stay props with today's values as defaults.
- **Styled** has `Movement`, a token naming what a motion is for, resolved by the theme to its
  curve: `Settle` (something landing where layout puts it: a glide, a dropped item, dock panes
  moving), `Enter` and `Leave` (a sheet, a back slide), `Rubber` (overscroll), and others only as a
  component needs one. (`Motion` is already the reduced-motion preference, which still overrides
  every token: under `Motion::Instant`, `beui-view` `animation.rs`, they land at once.)
- **Styled components** pass the token their motion is for and take no motion prop of their own.
  Add a token prop to one only when it needs a second look.
- The drag engine's reaches, snap radius, throw speed and edge band
  (plans/declarative_dragging.md) are distances, so behaviour: defaults in unstyled, not tokens.

## Steps

1. `Movement` and its theme values, matching today's curves so nothing changes on screen.
2. Make each curve above a required prop on its unstyled component, passed from styled. Unstyled
   components used outside styled (the dock's unstyled layer, beui-demo) pass values.
3. Add each curve to decisions/beui_unstyled_layer_has_no_appearance_defaults.md's list as it
   becomes required; a theme test that every `Movement` resolves; components.md's Timers and animation
   names the tokens.
