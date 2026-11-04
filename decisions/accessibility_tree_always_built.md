# beui builds the accessibility tree whether or not a screen reader asks for it

beui keeps the accessibility tree built all the time, instead of only while an assistive
technology is reading it.

## Why

It is wanted often, so the way to make it cost less is to make building it cheaper.

## Rejected

- Building the accessibility tree only while a screen reader asks for it: switching it off
  saved about 4%. Kept in reserve for when nothing better is left.
