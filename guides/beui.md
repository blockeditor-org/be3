# Beui

Beui is BE3's user interface toolkit, retained-mode and fine-grained reactive
like SolidJS. A `Document` owns a persistent tree of nodes; `view!` builds that
tree once, and signals update the nodes in place afterwards. There is no
virtual DOM, no diffing and no per-frame rebuild: a program keeps its
`Document` and calls `Document::show` each frame to dispatch input, lay out,
publish accessibility and paint. A component body runs **once**; the closures
and effects it leaves behind are what run again.

The component catalog is `crates/beui-demo` (in be-host, the block type beui
demo, editor `crates/be-app-example-beui-gallery`): a page per group of styled,
unstyled and base components, each sample shown beside its source (`#[sample]`
gives a component a `Name::SOURCE`). New components go there too.

[reactive.md](reactive.md) is the reference for signals, attribute syntax,
children and render props, bindings, keyed lists, scopes and context.

## Writing components

- Every function that builds part of a view is a `#[component]` named in
  `CamelCase`, including helpers that return a run of children. Without the
  attribute it would build into its caller's scope; `./scripts/verify` rejects
  a function containing `view!` that lacks it (test code, methods taking
  `self`, and deferred render callbacks are exempt). Functions that build no
  view (a colour from tokens and state) are plain functions.
- A component usually computes its signals, memos and callbacks, then ends with
  one `view!`. `view!` builds nodes the moment it runs, so a subtree built into
  a local and used conditionally is in the document either way.
- Control flow goes in the view: `Show` (builds when true, disposes when
  false), `ShowKeepAlive` (builds once, pauses while hidden), `Dynamic`,
  `Keyed` and `ForEach` (one keyed child per item). They build no node of their
  own; they fill a run in their parent's slot.
- Use a component as a tag: `<Button label="Save" on_click={save} />`. The
  generated builder is not an API; only the macro checks props, routes `@`
  modifiers and keeps render props unbuilt. To pass a component around, pass a
  `Render`/`RenderFn` closure that writes a `view!`.
- Bind signals and memos to props rather than reading them while building. Key
  a `ForEach` by item identity, never by content, so rows keep their nodes,
  focus and caret. State a control both shows and changes is one `Binding<T>`
  prop.

[beui/core.md](beui/core.md#writing-views) has the details: slots and child
types, value components, components in traits, props and callbacks.

### Imports

Program code imports `beui::reactive::*` (the framework and base nodes) and
`beui::styled::*` (components), and names anything else
(`beui::unstyled::X`, `beui::styled::theme::X`) on its own line.
`beui::primitives` is for beui's own crates, its tests and beui-demo's base
pages; the source lint rejects it elsewhere, since primitives take raw colours
and pixels where styled components take tokens.

## Crates

`beui` is the facade every program and editor depends on; it re-exports the
family as `beui::reactive`, `beui::styled`, `beui::unstyled`, `beui::dock`, ...,
plain values at its root (`beui::Sides`), and the native document's direct API
(`Document`, `with_document`, `with_reactive_scope`) as `beui::native`. It has
no features and no engine (renderer, fonts, inspector, drive, platforms), so
editing the engine rebuilds no editor
([decision](../decisions/beui_facade_excludes_the_engine.md)). `beui-host` is
beui plus the engine, for whatever runs documents: `beui_host::native` adds
`build` (a document with the inspector and on-screen keyboard), fonts,
`testing`, `drive`, `Headless`, renderers and the runners. Its features pick
the runner: `window` (default) or `web`.

Inside the family crates name each other directly, and the component crates
declare `extern crate beui_view as beui;` so the `::beui::reactive` paths the
macros expand to resolve. From the bottom up:

| Crate | Owns |
| --- | --- |
| `be-lib-ui-keys`, `be-lib-ui-preferences`, `be-lib-ui-types` | keys, preferences and the input and window vocabulary beui shares with the app protocol; serde only, generated from WIT |
| `beui-types` | plain values with no nodes in them, shared by both cores: colours, images, layout and text values, pointer and gesture values, the `drive` protocol, `icons` |
| `beui-tree` | `#[component]` and `view!`'s runtime, `Prop`, child slots, control flow; knows nothing of documents, so it can build any tree |
| `beui-macros` | the `view!` and `#[component]` grammar (the formatter's twin is `be-tool-fix-rust-source/src/views.rs`; change both) |
| `beui-core` | the native core: `Document`, `Context`, retained nodes, layout, input dispatch, accessibility, paint and damage, fonts and images interfaces, the `Program` contract |
| `beui-core-html` | the HTML core, built on `beui-html` |
| `beui-view` | the API both cores share: `beui::reactive`, `beui::primitives`, with a body per core |
| `beui-html` | HTML elements as components, serialized to strings or mounted as live DOM |
| `beui-components-unstyled`, `beui-components-styled` | `beui::unstyled` (and `beui::datetime`), `beui::styled` |
| `beui-dock`, `beui-dock-host`, `beui-dock-state` | docking: what an editor declares, what lays it out and draws it, the layout model |
| `beui-keyboard` | the on-screen keyboard |
| `beui-inspector` | the inspector, simulated screen reader, mouse and keyboard |
| `beui-draw`, `beui-renderer-wgpu` | gpu drawings, and the wgpu renderer |
| `beui-runner` | the `Runner`, `Platform`, `PlatformServices`, `Layer` and `Renderer` contracts, `RunOptions`, the `BEUI_*` environment |
| `beui-platform-*`, `beui-layer-*` | platforms (winit, android, web, drm, headless) and layers (space, xr, drive) |
| `beui-font-freetype` | shaping and rasterizing, and the bundled fonts |
| `be-sdk-beui-platform` | the platform an app module's views run on |
| `beui-paint-format`, `beui-paint-snapshot` | paintings and the snapshot assertions |
| `beui-tests` | screens and scripts that must behave the same on both cores |

## Two cores

The native core (`beui-core`) lays out, focuses, hit-tests and paints retained
nodes itself, on every native platform and in be-host's web build. The HTML
core (`beui-core-html`) makes each primitive a DOM element and lets the browser
lay out, focus, select and paint. Both sit behind one API in beui-view, so
unstyled, styled and everything above are the same source on both
([decision](../decisions/beui_has_an_html_core_on_the_web.md)). `--cfg
beui_html` picks the core at compile time; code that must differ checks
`beui::CORE`. [beui/core.md](beui/core.md#cores) has the contract both keep and
how to test on both.

## The other beui guides

- [beui/core.md](beui/core.md): writing views, the frame, layout and
  measurement, coordinates, base components, the HTML core and `beui-html`,
  testing.
- [beui/components.md](beui/components.md): the unstyled and styled layers and
  tokens, controls, scrolling and long lists, forms, drag and drop, timers,
  drawing with the gpu, writing components.
- [beui/text.md](beui/text.md): `Text`, styled text, fonts and icons, text
  areas, rich text, selecting text.
- [beui/overlays.md](beui/overlays.md): layers, placement, policies,
  dismissing, focus, modals and dialogs, toasts, trees, escaping layers.
- [beui/docking.md](beui/docking.md): the dock and window manager, and windows
  and window hosts.
- [beui/programs.md](beui/programs.md): standalone programs, `Root`,
  `RunOptions`, layers, drivers, the inspector and simulation tools, beui in
  an app.
- [beui/platforms.md](beui/platforms.md): the runner, input, the clipboard,
  renderers, each platform, displays under drm.
- [beui/space.md](beui/space.md): the Space VR simulator, XR headsets, the
  on-screen keyboard.
- [beui_keyboard.md](beui_keyboard.md): focus and keyboard input.
