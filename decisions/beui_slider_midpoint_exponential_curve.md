# beui slider midpoint scales use an exponential curve

`SliderScale::Midpoint(value)` curves a slider so the track's centre reads `value`, and the curve is exponential rather than a power of the position.

## Why

An exponential curve keeps the fine end useful without giving the whole of it away: the inspector's blur slider (to 120 px, midpoint 12) reads 3 px, 12 px and 39 px at the quarters and spends a tenth of the track below one pixel.

## Rejected

- A power curve through the same midpoint: it spends a quarter of the blur slider's track below one pixel.
