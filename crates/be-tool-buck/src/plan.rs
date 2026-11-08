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
}

/// What the host's plan leaves out: the wasm-only crates.
const HOST_EXCLUDES: &[Pick] = &[
    AppsWithoutExamples,
    Named("be-sdk"),
    Named("be-sdk-beui"),
    Named("be-wasm-gpu-shim"),
    Named("beui-web-demo"),
    Named("be-server-portal-ui"),
    Named("be-server-bin"),
    Named("beui-demo-html"),
    Named("be-tool-paint-review-web"),
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
    Named("be-host-accounts-ui"),
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
    },
    // be-host, beui's DOM demo, the paint review page and beui-html's live DOM
    // for the web.
    Plan {
        name: "wasi",
        mode: Mode::Build,
        triple: "wasm32-wasip1-threads",
        packages: Packages::Only(&[
            Named("be-host"),
            Named("beui-web-demo"),
            Named("be-tool-paint-review-web"),
            Named("beui-html"),
        ]),
        html: false,
        lib: true,
    },
    // The apps, the libraries apps are linked against and the crates that
    // test linking them (wasm_library and linked_app in buck/wasm/defs.bzl),
    // beui's demo and the paint review, whose tests paint as an app's do: a
    // plan of their own, since they want a different wgpu.
    Plan {
        name: "wasi-guest",
        mode: Mode::Test,
        triple: "wasm32-wasip1-threads",
        packages: Packages::Only(&[
            Prefix("be-app-", &[]),
            Prefix("be-sdk-library-", &[]),
            Prefix("be-tool-tls-diamond-", &[]),
            Named("be-sdk"),
            Named("be-sdk-beui"),
            Named("beui-demo"),
            Named("be-tool-paint-review"),
        ]),
        html: false,
        lib: false,
    },
    // The games, every app's type module (be-type-bundle runs them in process
    // on the host), the gpu shim, the remap module and the pure guests.
    Plan {
        name: "wasm32",
        mode: Mode::Build,
        triple: "wasm32-unknown-unknown",
        packages: Packages::Only(&[
            Prefix("tabletop-rules-", &[]),
            Prefix("be-type-", &["be-type-bundle"]),
            Named("be-wasm-gpu-shim"),
            Named("be-lib-remap-module"),
            Named("be-wasm-guest-echo"),
        ]),
        html: false,
        lib: false,
    },
    // beui's HTML core in the browser, with the crates whose native side runs
    // only on the native core, such as the shared tests' fixtures program, and
    // beui's demo in the browser.
    Plan {
        name: "wasi-html",
        mode: Mode::Build,
        triple: "wasm32-wasip1-threads",
        packages: Packages::Only(&[All(HTML), Named("beui-tests"), Named("beui-demo-html")]),
        html: true,
        lib: true,
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
