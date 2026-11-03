# beui lists are written with props, not Row/Column aliases

A line of children is always a `List` with `direction`, `align`, `justify` and `spacing` props, for example `<List direction=Direction::Horizontal spacing=8.0>`. There are no named aliases per combination.

## Why

Reading the props at the call site says what the list does; aliases make readers remember which names exist and what each one sets.

## Rejected

- `Row`, `Column` and `CenteredRow` one-line aliases: each was just a combination of `List` props, and they were removed.
