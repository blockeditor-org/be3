# A scroll with stops runs beui's own motion on the HTML core

On the HTML core a `Scroll` without `snaps` is the browser's own scroll (`BrowserScroll`); one with `snaps` (today only a sheet's) runs beui's own `Scrolling` there as natively, an `Offset` it moves.

## Why

A sheet needs what a browser scroll cannot give: its rise and settle on a spring, a fling through its content carried into the sheet, a back gesture's speed carried into its slide, a drag that starts beside it (the space above), and a wheel that never lowers it. With the browser's scroll snap each of these was a web-only stand-in (snap markers, a debounce for `scrollend`, a drag callback that jumped to the nearest stop), and a sheet moved differently on the web. Running beui's motion leaves a sheet no code of its own on the web.

## Rejected

- CSS scroll snap (`scroll-snap-type`, a marker at each stop): it settles at once or on the browser's own curve, cannot start from a given speed, and needed its own code beside the native path.
