# beui has two cores, chosen at compile time: its own natively, HTML on the web

Native platforms run beui's own core (`beui-core`'s retained nodes and the `beui-view` primitives over them). The web build of beui runs an HTML core (`beui-core-html`) that implements the same base components with real DOM elements, chosen by the `--cfg beui_html` flag per buck platform. Unstyled and styled are shared and reach base only through `beui-view`; where a real HTML element replaces beui's own, unstyled switches on `beui_view::CORE` at that one component, as a deliberate choice, never to work around a missing base primitive. No feature exists on only one core, and native must not regress: virtual scrolling stays frame-perfect, measurement and scroll anchoring stay synchronous; callers stop assuming an answer arrives in the same frame.

## Why

- The browser then does layout, focus, input and selection, which buys server-side rendering with partial hydration, browser-owned focus, autofill and password managers, find-in-page, translation and screen readers on real DOM text, and a smaller download.
- Every base primitive costs two implementations, so the base layer must stay small.
- On the web `ResizeObserver` callbacks run after layout and before paint in the same frame, so DOM virtualization can be jump-free too.

## Rejected

- A cargo feature to pick the core: buck takes features from cargo's plan per platform, so a rustc cfg per platform is used instead.
- Keeping beui's own scrolling on the web: unstyled's `Scroll` is a real `overflow: auto` scroller on the HTML core (`BrowserScroll`), for compositor-thread scrolling and the browser's wheel, touch, keys and scrollbar.
- Plain single-line fields drawn by beui on the web: a plain `TextInput` is a real `<input>` there, `Form` a `<form method=post>`, `Button submit` a submit button and `Button href` a link, so a server-rendered page posts and navigates before wasm loads.
