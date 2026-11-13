# Motion tokens

Depends on: none.

Styled components take design tokens for space, radii, text and colour (`beui-components-styled`
`tokens.rs`), but there is none for motion, so unstyled components hard-code their curves, against
the rule that base components take concrete values and know nothing about the theme:

- `sheet.rs`: `ENTER_SECONDS`, `LEAVE_SECONDS`, `RETURN_SECONDS`, `HANDOFF_SECONDS`, `OutCubic`;
- `back_slide.rs`: its enter, leave and return durations and easings;
- `rubber_band.rs`: `SCROLL_SPRING`, `WINDOW_SPRING`;
- `beui-dock-host`: `GAP_SECONDS`, `SETTLE`, `SIDEBAR_SECONDS`;
- `beui-types`: `GLIDE_SPRING`.

## Shape

- **Unstyled** components take each curve as a prop, a `Spring` or a duration with an `Easing`
  (`beui-types` `animation.rs`), with no default.
- **Styled** has `Movement`, a token naming what a motion is for, resolved by the theme to its
  values: `Settle` (something landing where layout puts it: a glide, a dropped item, dock panes
  moving), `Enter` and `Leave` (a sheet, a back slide), `Rubber` (overscroll), and others only as a
  component needs one. (`Motion` is already the reduced-motion preference, which still overrides
  every token: under `Motion::Instant` they land at once.)
- **Styled components** pass the token their motion is for and take no motion prop of their own.
  Add a token prop to one only when it needs a second look.
- The drag engine's reaches, snap radius and throw speed (plans/declarative_dragging.md) are
  tokens of the same layer, distances rather than curves.

## Steps

1. `Movement` and its theme values, matching today's constants so nothing changes on screen.
2. Move each hard-coded curve above to a prop on the unstyled component, passed from styled.
   Unstyled components used outside styled (the dock's unstyled layer, beui-demo) pass values.
3. A theme test that every `Movement` resolves; components.md's Timers and animation names the
   tokens.
