# The inspector's blur filter is a dual Kawase chain

The blur in beui's filter pass (`Context::apply_filter`, `Group::Filter`) is a dual Kawase chain: the frame is halved a level at a time, then tented back up, with the number of levels taken from the radius.

## Why

Its cost does not grow with the radius (120 points costs no more than 8), so the Sim tab's slider can go as wide as it likes without the frame rate following it. Its reach is bounded per level, which lets the damage be widened by a known amount and every pass be scissored to it.
