# Common commands

This guide is for people as well as agents. It sorts the ways to run things by who can use them:
a person at a machine with displays, a device or a headset, and an agent in a container with
none of them, then lists every target. guides/running/ says how to run and drive each platform.
`./scripts/buck run` passes the environment on to the program, so a `BEUI_*` variable before it
reaches the program.

## For a person

```
# the host in a window, with every app beside it
./scripts/buck run //crates/be-host:native

# the desktop shell in a window
./scripts/buck run //crates/be-host:native -- --desktop

# the host in the VR simulator: its windows as panels in a 3D scene, in a window
BEUI_LAYERS=space ./scripts/buck run //crates/be-host:native

# the host on an OpenXR headset (SteamVR or Monado running); the window is the headset's mirror
BEUI_LAYERS=xr ./scripts/buck run //crates/be-host:native

# the desktop as a login session on this machine's displays, from a virtual terminal with no
# other display server on it (Linux)
BEUI_PLATFORM=drm ./scripts/buck run //crates/be-host:native -- --desktop

# install it as a login session the login screen offers, under /usr/local or PREFIX (Linux)
./scripts/buck run //crates/be-host:install-login-session [-- PREFIX]

# build the APK, install it on the connected device and start it
./scripts/buck run //crates/be-host:android -- --install

# the host's web build and be-server, on http://127.0.0.1:8080
./scripts/buck run //crates/be-host:web

# beui's demo in a window; any beui program takes BEUI_LAYERS too
./scripts/buck run //crates/beui-demo:native
```

`xr` is space on a headset, in space's place: name `xr` alone, not `space,xr`, which stacks a
second space under it. With no headset it waits, with a notice over the simulator, and finds one
started or plugged in later. `drm` cannot show space on a headset, and Android reads no `BEUI_*`
variables. guides/beui/space.md has both layers; the login session and its install are
guides/desktop/login_session.md, Installing it.

## For an agent

An agent cannot use the displays (`drm`), a headset, an Android device or the emulator (no KVM in
a cloud container, guides/running/android.md); it runs everything headless and drives it.

```
# the host headless, signed in with a workspace open, for `drive` (guides/running/drive.md)
./scripts/buck run //crates/be-host:dev [-- --fresh | --stop]
source ~/.cache/be3/dev/env

# the desktop shell headless
./scripts/buck run //crates/be-host:dev -- --desktop

# the VR simulator headless, with drive taps under space and the host: `drive --into=space ...`
# walks and clicks through space, `drive xr left|right ...` works a headset's controllers
BEUI_LAYERS=space ./scripts/buck run //crates/be-host:dev

# the host's web build in a headless Chromium (guides/running/web.md)
./scripts/buck run //crates/be-host:web-dev

# beui's demo and the launcher, headless
./scripts/buck run //crates/beui-demo:dev
./scripts/buck run //crates/be-tool-launcher:dev

# build the APK without a device
./scripts/buck run //crates/be-host:android
```

An agent shows a visual change with a painting (guides/testing_a_gui.md).

## Every target

| Command | What it does |
|---|---|
| `./scripts/verify` | rustc's check pass over the first-party targets on Linux x86_64 nothing else compiles, autofixes, lints, tests and app tests; `--check` writes nothing, `--lint`, `--tests`, `--app-tests` run one part; every run also brings `buck/sysroot/packages.bzl` up to date with `buck/sysroot/BUCK` |
| `./scripts/buck bxl --keep-going //buck/dev/verify.bxl:main -- --host linux_x86_64 --part tests` | one part of what `./scripts/verify` builds, without reading it afterwards: `lint`, `tests`, `app-tests` (guides/buck2.md, What verify builds) |
| `./scripts/ci` | what CI runs: `./scripts/verify` and crates/be-tool-bootstrap's release (guides/bootstrap.md), and `--previews BASE OUT` the paint previews |
| `./scripts/test adding [--update]` | the tests whose full names contain `adding` (or a module path such as `media::tests`), in every crate. A path matches inside other crates' paths too (`desktop::tests` also runs be-host's `compositor::desktop::tests`), so name the crate's target (next row) to run one crate's |
| `./scripts/test //crates/be-app-desktop:test [adding] [--update]` | one crate's tests, or those of them whose names contain `adding`; `--update` accepts their paintings |
| `./scripts/buck run //crates/be-host:native` | the host, with every app beside it |
| `./scripts/test //crates/be-host-tests:test [--update]` | the host tests: be-host headless with the editors each test names, driven and painted as a whole |
| `./scripts/buck test //crates/be-lib-geometry:test` | one crate's tests through buck2 test, which prints only a failing test's output (`./scripts/test` runs the same and prints every test's); `build //crates/be-lib-geometry:test[run]` is the action verify runs, and `build //crates/be-lib-geometry:fix` the crate's autofixes |
| `./scripts/buck build //crates/be-host:native-dist --out DIR` | a platform's release of the host; `//crates/be-server-bin:dist` is the server's |
| `./scripts/buck build //crates/be-host:apps --out DIR` | the apps alone, shared by every platform |
| `./scripts/buck build //crates/be-host:web --out DIR` | the web bundle with every app (`:web-dist` without) |
| `./scripts/buck run //crates/be-host:web` | the web bundle and `be-server`, on http://127.0.0.1:8080 |
| `./scripts/buck run //crates/be-host:android -- --install` | the APK, signed with this machine's key, installed and started (`build :android-dist` is the build helper's unsigned template, which it signs with CI's key) |
| `./scripts/buck run //crates/beui-demo:native` | beui's demo in a window |
| `./scripts/buck run //crates/beui-host:example-survey` | a crate example; every example is `example-<name>` |
| `./scripts/buck run //crates/be-tool-bench:suite -- run [NAME]` | every benchmark, built in release (guides/performance.md) |
| `./scripts/buck run //crates/beui-web-demo:web` | beui's demo in a browser, drawn with DOM elements, on http://127.0.0.1:8070 |
| `./scripts/buck run //crates/be-tool-paint-review-web:web -- 127.0.0.1:8075 DIR` | the page reviewing a pull request's paintings, serving the `HASH.json` lists and `HASH.webp` images in DIR beside it on http://127.0.0.1:8075/site/dev/index.html?changes=HASH |
| `./scripts/buck run //:fuzz` | every fuzz target, taking turns with every core (`BE3_FUZZ_SLICE` seconds each, 600 by default), until it is stopped |
| `./scripts/buck run //crates/be-lib-sequence:fuzz-sequence -- FILE` | one fuzz target; with an input file, runs it once |
| `./scripts/buckify` | brings `Cargo.lock` and the crates' facts (each `crates/<crate>/PACKAGE`, `buck/cargo/members.bzl`, `third-party/rust/crates.bzl`) up to date with the manifests (the bootstrap's lock step, then the plans on a worker); `BE3_BOOTSTRAP=source` until a bootstrap with it is pinned |
| `./scripts/buck run //:rust-project` | writes `rust-project.json` for rust-analyzer |
| `./scripts/buck run //:pdfium -- REVISION` | regenerates `third-party/pdfium/sources.bzl` from PDFium's GN build at REVISION (a commit, or a branch such as `chromium/8046`), which is how PDFium is updated; it checks PDFium and its DEPS out under `target/pdfium` and needs git, curl and python3 |

guides/buck2.md has the platforms and profiles these build with (`--target-platforms`, `-m release`).
