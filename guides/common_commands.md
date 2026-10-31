# Common commands

This guide is for people as well as agents. It sorts the ways to run things by who can use them:
a person at a machine with displays, a device or a headset, and an agent in a container with
none of them. AGENTS.md's Commands section has the everyday ones (`./scripts/verify`,
`./scripts/test`, `:dev`, `rasterize`); guides/running/ says how to run and drive each platform.
`./scripts/buck run` passes the environment on to the program, so a `BEUI_*` variable before it
reaches the app.

## For a person

```
# the app in a window, with every plugin beside it
./scripts/buck run //crates/block-app:native

# the desktop shell in a window
./scripts/buck run //crates/block-app:native -- --desktop

# the app in the VR simulator: its windows as panels in a 3D scene, in a window
BEUI_LAYERS=space ./scripts/buck run //crates/block-app:native

# the app on an OpenXR headset (SteamVR or Monado running); the window is the headset's mirror
BEUI_LAYERS=xr ./scripts/buck run //crates/block-app:native

# the desktop as a login session on this machine's displays, from a virtual terminal with no
# other display server on it (Linux)
BEUI_PLATFORM=drm ./scripts/buck run //crates/block-app:native -- --desktop

# install it as a login session the login screen offers, under /usr/local or PREFIX (Linux)
./scripts/buck run //crates/block-app:install-login-session [-- PREFIX]

# build the APK, install it on the connected device and start it
./scripts/buck run //crates/block-app:android -- --install

# the web app and be-server, on http://127.0.0.1:8080
./scripts/buck run //crates/block-app:web

# beui's demo in a window; any beui program takes BEUI_LAYERS too
./scripts/buck run //crates/beui-demo:native
```

`xr` is space on a headset, in space's place: name `xr` alone, not `space,xr`, which stacks a
second space under it. With no headset it waits, with a notice over the simulator, and finds one
started or plugged in later. `drm` cannot show space on a headset, and Android reads no `BEUI_*`
variables. guides/beui.md ("Space: the VR simulator layer", "Headsets") has both layers; the
login session and its install are guides/desktop/linux.md.

## For an agent

An agent cannot use the displays (`drm`), a headset, an Android device or the emulator (no KVM in
a cloud container, guides/running/android.md); it runs everything headless and drives it.

```
# the app headless, signed in with a workspace open, for `drive` (guides/running/drive.md)
./scripts/buck run //crates/block-app:dev [-- --fresh | --stop]
source ~/.cache/be3/dev/env

# the desktop shell headless
./scripts/buck run //crates/block-app:dev -- --desktop

# the VR simulator headless, with drive taps under space and the app: `drive --into=space ...`
# walks and clicks through space, `drive xr left|right ...` works a headset's controllers
BEUI_LAYERS=space ./scripts/buck run //crates/block-app:dev

# the web app in a headless Chromium (guides/running/web.md)
./scripts/buck run //crates/block-app:web-dev

# beui's demo and the launcher, headless
./scripts/buck run //crates/beui-demo:dev
./scripts/buck run //crates/be-launcher:dev

# build the APK without a device
./scripts/buck run //crates/block-app:android
```

Tests and paintings are how an agent shows a visual change: `./scripts/test NAME --update` and
`rasterize` (AGENTS.md), guides/testing_a_gui.md.

## Every target

| Command | What it does |
|---|---|
| `./scripts/verify` | rustc's check pass over every first-party target on Linux x86_64 and the plugins' `wasi_guest`, autofixes, lints, tests and plugin tests; `--check` writes nothing, `--lint`, `--tests`, `--plugin-tests` run one part |
| `./scripts/ci` | what CI runs: `./scripts/verify` and crates/bootstrap's release (guides/bootstrap.md), and `--previews BASE OUT` the paint previews |
| `./scripts/test adding [--update]` | the tests whose full names, module path included, contain `adding` (or `media::tests`), in every crate that has one, with all their output; it finds each crate's `:test` and runs it on a worker (below). A path matches anywhere in another crate's paths too: `desktop::tests` also runs block-app's `compositor::desktop::tests`, so name the crate's target (next row) to run one crate's |
| `./scripts/test //crates/editors/desktop:test [adding] [--update]` | one crate's tests, or those of them whose names contain `adding`; `--update` accepts their paintings |
| `./scripts/buck run //crates/block-app:native` | the app, with every plugin beside it |
| `./scripts/test //crates/app-tests:test [--update]` | the app tests: block-app headless with every plugin, driven and painted as a whole |
| `./scripts/buck build //crates/block-app:native-dist --out DIR` | a platform's release of the app; `//crates/be-server-bin:dist` is the server's |
| `./scripts/buck build //crates/block-app:plugins --out DIR` | the plugins alone, shared by every platform |
| `./scripts/buck build //crates/block-app:web --out DIR` | the web bundle with every plugin (`:web-dist` without) |
| `./scripts/buck run //crates/block-app:web` | the web bundle and `be-server`, on http://127.0.0.1:8080 |
| `./scripts/buck run //crates/block-app:android -- --install` | the APK, signed with this machine's key, installed and started (`build :android-dist` is the build helper's unsigned template, which it signs with CI's key) |
| `./scripts/buck run //crates/beui-demo:native` | beui's demo in a window |
| `./scripts/buck run //crates/beui-host:example-survey` | a crate example; every example is `example-<name>` |
| `./scripts/buck run -m release //crates/beui-host:example-text-bench -- [NAME] [--frames N]` | beui's text benchmarks: per-frame times for resizing wrapped text and labels, editing a paragraph or one span, and typing into a text area; NAME picks the scenarios whose names contain it. Compare a change by running it on the change and on its base |
| `./scripts/buck run -m release //crates/beui-demo:example-open-bench -- [PAGE]` | times opening each beui-demo page from its catalog row and counts its allocations (guides/performance.md) |
| `./scripts/buck run //crates/beui-web-demo:web` | beui's demo in a browser, drawn with DOM elements, on http://127.0.0.1:8070 |
| `./scripts/buck run //crates/paint-review-web:web -- 127.0.0.1:8075 DIR` | the page reviewing a pull request's paintings, serving the `HASH.json` lists and `HASH.png` images in DIR beside it on http://127.0.0.1:8075/site/dev/index.html?changes=HASH |
| `./scripts/buck run //:fuzz` | every fuzz target, taking turns with every core (`BE3_FUZZ_SLICE` seconds each, 600 by default), until it is stopped |
| `./scripts/buck run //crates/sequence:fuzz-sequence -- FILE` | one fuzz target; with an input file, runs it once |
| `./scripts/buck run //:buckify` | regenerates `buck/cargo/crates.bzl` and `Cargo.lock` from the manifests |
| `./scripts/buck run //:rust-project` | writes `rust-project.json` for rust-analyzer |
| `./scripts/buck run //:lock-sysroot` | re-resolves `buck/sysroot/packages.bzl` |
| `./scripts/buck run //:pdfium -- REVISION` | regenerates `third-party/pdfium/sources.bzl` from PDFium's GN build at REVISION (a commit, or a branch such as `chromium/8046`), which is how PDFium is updated; it checks PDFium and its DEPS out under `target/pdfium` and needs git, curl and python3 |

guides/buck2.md has the platforms and profiles these build with (`--target-platforms`, `-m release`).
