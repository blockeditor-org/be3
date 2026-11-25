//! buckify's plans (buck/cargo/buckify.bzl): cargo asked for every build buck2
//! stands in for, in a copy of the workspace crates/be-tool-cargo-offline has
//! made that cargo plans offline, and the facts generated from them into
//! OUT/files at their paths from the repository's root.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

use crate::generate::{Inputs, generate};

const USAGE: &str = "usage: plan OUT WORKSPACE NIGHTLY [PLAN=TRIPLE]...";

/// Whether cargo is asked to build or to test: `cargo test` where there are
/// tests, so the plan has the dev-dependencies.
#[derive(Clone, Copy)]
enum Mode {
    Build,
    Test,
}

/// Packages a plan names, by what their names are.
enum Pick {
    Named(&'static str),
    /// Every workspace crate whose name starts with this, but those named in
    /// the second.
    Prefix(&'static str, &'static [&'static str]),
    /// Every app with no examples. An editor with examples is built for the
    /// host as well, where they run (editor() in buck/wasm/defs.bzl).
    AppsWithoutExamples,
    All(&'static [Pick]),
}

use Pick::{All, AppsWithoutExamples, Named, Prefix};

enum Packages {
    /// `--workspace`, but these.
    Workspace(&'static [Pick]),
    Only(&'static [Pick]),
}

pub(crate) struct Plan {
    pub(crate) name: &'static str,
    mode: Mode,
    triple: &'static str,
    packages: Packages,
    /// Under the cfg that picks beui's HTML core (buck/constraints:html).
    /// cargo evaluates [target.'cfg(beui_html)'.dependencies] against
    /// RUSTFLAGS, so each core's dependencies drop out of the other's plans.
    html: bool,
    /// The libraries alone, `--lib`.
    lib: bool,
    /// Whether the crates' binaries and examples are built for the platform. Without them
    /// `cargo test` still plans them; the facts leave them out, so a crate here only for its
    /// library and tests, such as beui-demo, runs its programs only where they are planned.
    programs: bool,
}

/// Whether a platform's plan builds its crates' binaries and examples: every plan but those
/// that say not, and the cross platforms, which are the host's plan for another triple.
pub(crate) fn builds_programs(platform: &str) -> bool {
    PLANS
        .iter()
        .find(|plan| plan.name == platform)
        .is_none_or(|plan| plan.programs)
}

/// What the host's plan leaves out: the wasm-only crates.
const HOST_EXCLUDES: &[Pick] = &[
    AppsWithoutExamples,
    Named("be-sdk"),
    Named("be-sdk-beui"),
    Named("be-wasm-gpu-shim"),
    Named("be-server-portal-ui"),
    Named("be-server-bin"),
    Named("be-tool-paint-review-web"),
    Named("be-wasm-root-web-testing"),
    // PDFium, which it links, is built only for the apps' platform.
    Named("be-worker-pdf-render"),
];

/// The crates that build on beui's HTML core, for the browser and for the host.
const HTML: &[Pick] = &[
    Named("beui"),
    Named("beui-dock"),
    Named("beui-dock-host"),
    Named("beui-dock-state"),
    Named("beui-keyboard"),
    Named("beui-view"),
    Named("beui-html"),
    Named("beui-core-html"),
    Named("beui-components-unstyled"),
    Named("beui-components-styled"),
    Named("be-root-accounts-ui"),
    Named("be-server-portal-ui"),
];

/// Every build buck2 stands in for, as cargo would be asked for it, and the
/// one place that says which crates each platform builds: defs.bzl makes a
/// crate's targets incompatible with every plan that leaves it out. Every plan
/// names its target, so what cargo builds for the machine running the build -
/// proc macros, build scripts and what they depend on - is told apart from the
/// rest. The host's plan is made again for each cross platform (PLAN=TRIPLE).
pub(crate) const PLANS: &[Plan] = &[
    // The host, without the wasm-only crates.
    Plan {
        name: "linux-x86_64",
        mode: Mode::Test,
        triple: "x86_64-unknown-linux-gnu",
        packages: Packages::Workspace(HOST_EXCLUDES),
        html: false,
        lib: false,
        programs: true,
    },
    // be-host, beui's demo drawn with wgpu and the entry it runs through, the paint
    // review page, beui-html's live DOM and the page that tests the root entry for
    // the web.
    Plan {
        name: "wasi",
        mode: Mode::Build,
        triple: "wasm32-wasip1-threads",
        packages: Packages::Only(&[
            Named("be-host"),
            Named("beui-demo"),
            Named("beui-host-window"),
            Named("be-tool-paint-review-web"),
            Named("beui-html"),
            Named("be-wasm-root-web-testing"),
        ]),
        html: false,
        lib: true,
        programs: true,
    },
    // Every guest (guides/app_libraries.md): the apps, the libraries apps are linked against
    // (wasm_library in buck/wasm/defs.bzl), the echo guest, pure and staged as an app, and its
    // worker, the apps' workers, beui's demo, the paint review, whose tests paint as an app's do,
    // the root guests linked as apps are, be-root and the testing root
    // (decisions/a_root_links_beui_as_an_app_does.md), and the pure guests: the games, every
    // app's type module (be-type-bundle runs them in process on the host), the remap module and
    // the pure root guest be-wasm-root-host's tests run. A plan of their own, since they want a
    // different wgpu from the web bundle's.
    Plan {
        name: "wasi-guest",
        mode: Mode::Test,
        triple: "wasm32-wasip1",
        packages: Packages::Only(&[
            Prefix("be-app-", &[]),
            Prefix("be-sdk-library-", &[]),
            Named("be-wasm-guest-echo"),
            Named("be-sdk-worker-echo"),
            Prefix("be-worker-", &[]),
            Named("be-sdk"),
            Named("be-sdk-beui"),
            Named("beui-demo"),
            Named("be-tool-paint-review"),
            Named("be-root"),
            Named("be-sdk-root-testing"),
            Prefix("tabletop-rules-", &[]),
            Prefix("be-type-", &["be-type-bundle"]),
            Named("be-lib-remap-module"),
            Named("be-wasm-root-testing"),
        ]),
        html: false,
        lib: false,
        programs: false,
    },
    // The browser's gpu shim, which wasm-bindgen binds to the page: wasm-bindgen code for
    // wasm32-unknown-unknown, not a guest.
    Plan {
        name: "wasm32-bindgen",
        mode: Mode::Build,
        triple: "wasm32-unknown-unknown",
        packages: Packages::Only(&[Named("be-wasm-gpu-shim")]),
        html: false,
        lib: false,
        programs: true,
    },
    // beui's HTML core in the browser, with the crates whose native side runs
    // only on the native core, such as the shared tests' fixtures program, and
    // beui's demo in the browser and the entry it runs through.
    Plan {
        name: "wasi-html",
        mode: Mode::Build,
        triple: "wasm32-wasip1-threads",
        packages: Packages::Only(&[
            All(HTML),
            Named("beui-tests"),
            Named("beui-demo"),
            Named("beui-host-window"),
        ]),
        html: true,
        lib: true,
        programs: true,
    },
    // And on the host, with be-server-bin, the server's executable, which
    // renders its account portal on that core.
    Plan {
        name: "linux-x86_64-html",
        mode: Mode::Test,
        triple: "x86_64-unknown-linux-gnu",
        packages: Packages::Only(&[All(HTML), Named("be-server-bin")]),
        html: true,
        lib: false,
        programs: true,
    },
];

/// A workspace crate, as the plans pick it.
pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) examples: bool,
}

impl Pick {
    fn names(&self, members: &[Member], into: &mut Vec<String>) {
        match self {
            Named(name) => into.push((*name).to_owned()),
            Prefix(prefix, except) => into.extend(
                members
                    .iter()
                    .filter(|member| {
                        member.name.starts_with(prefix) && !except.contains(&member.name.as_str())
                    })
                    .map(|member| member.name.clone()),
            ),
            AppsWithoutExamples => into.extend(
                members
                    .iter()
                    .filter(|member| member.name.starts_with("be-app-") && !member.examples)
                    .map(|member| member.name.clone()),
            ),
            All(picks) => {
                for pick in *picks {
                    pick.names(members, into);
                }
            }
        }
    }
}

fn members(metadata: &Value) -> Result<Vec<Member>, String> {
    let ids: BTreeSet<&str> = metadata["workspace_members"]
        .as_array()
        .ok_or("metadata has no workspace_members")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut members: Vec<Member> = metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?
        .iter()
        .filter(|package| package["id"].as_str().is_some_and(|id| ids.contains(id)))
        .map(|package| Member {
            name: package["name"].as_str().unwrap_or_default().to_owned(),
            examples: package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|target| {
                    target["kind"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|kind| kind == "example")
                }),
        })
        .collect();
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(members)
}

/// cargo's arguments for one plan.
pub(crate) fn cargo_arguments(plan: &Plan, triple: &str, members: &[Member]) -> Vec<String> {
    let mut arguments: Vec<String> = [
        match plan.mode {
            Mode::Build => "build",
            Mode::Test => "test",
        },
        "--target",
        triple,
    ]
    .map(str::to_owned)
    .to_vec();
    let mut names = Vec::new();
    match &plan.packages {
        Packages::Workspace(except) => {
            arguments.push("--workspace".to_owned());
            for pick in *except {
                pick.names(members, &mut names);
            }
            for name in names {
                arguments.extend(["--exclude".to_owned(), name]);
            }
        }
        Packages::Only(picks) => {
            for pick in *picks {
                pick.names(members, &mut names);
            }
            for name in names {
                arguments.extend(["-p".to_owned(), name]);
            }
        }
    }
    if plan.lib {
        arguments.push("--lib".to_owned());
    }
    // --unit-graph needs the nightly.
    arguments.extend(["--unit-graph", "-Z", "unstable-options", "--locked"].map(str::to_owned));
    arguments
}

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, workspace, nightly, cross @ ..] = arguments else {
        return Err(USAGE.to_owned());
    };
    let absolute = |path: &str| -> Result<PathBuf, String> {
        std::path::absolute(path).map_err(|error| format!("{path}: {error}"))
    };
    let out = absolute(out)?;
    let workspace = absolute(workspace)?;
    let nightly = absolute(nightly)?;
    let cargo = |arguments: &[String], html: bool| -> Result<Value, String> {
        let mut command = Command::new(nightly.join("bin/cargo"));
        command
            .args(arguments)
            .current_dir(&workspace)
            .env("RUSTC", nightly.join("bin/rustc"));
        if html {
            command.env("RUSTFLAGS", "--cfg beui_html");
        }
        serde_json::from_str(&crate::output(&mut command)?)
            .map_err(|error| format!("cargo {}: {error}", arguments.join(" ")))
    };
    let metadata = cargo(
        &[
            "metadata",
            "--all-features",
            "--format-version",
            "1",
            "--locked",
        ]
        .map(str::to_owned),
        false,
    )?;
    let members = members(&metadata)?;
    let mut plans = Vec::new();
    for plan in PLANS {
        plans.push((
            plan.name.to_owned(),
            cargo(&cargo_arguments(plan, plan.triple, &members), plan.html)?,
        ));
    }
    let host = &PLANS[0];
    for platform in cross {
        let (name, triple) = platform
            .split_once('=')
            .ok_or_else(|| format!("{platform} is not PLAN=TRIPLE"))?;
        plans.push((
            name.to_owned(),
            cargo(&cargo_arguments(host, triple, &members), false)?,
        ));
    }
    let inputs = Inputs {
        metadata,
        plans,
        lock: crate::read(&workspace.join("Cargo.lock").to_string_lossy())?,
        manifest: crate::read(&workspace.join("Cargo.toml").to_string_lossy())?,
    };
    for (path, text) in generate(&inputs)? {
        write(&out.join("files").join(&path), &text)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}
