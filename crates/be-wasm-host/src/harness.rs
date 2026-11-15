//! Running a crate's tests compiled to wasm (be-sdk-test-runner): once together, and when that
//! fails, each again on its own, since a panic ends a wasm run.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::wasi::{EXITED, Output, Wasi};
use crate::{Host, Running, Setup, Stored, wake::Wake};

const START: &str = "_start";
const MAIN: &str = "__main_void";
const CARRIED: [&str; 2] = ["--ignored", "--include-ignored"];
const PANICKED: &str = " panicked at ";
const ABORT: &str = "abort";
const UNREACHABLE: &str = "RuntimeError: unreachable";
const PANIC_MACHINERY: [&str; 5] = [
    "core::panicking::",
    "core::option::expect_failed",
    "core::option::unwrap_failed",
    "core::result::unwrap_failed",
    "std::panicking::",
];
const TEST_HARNESS: &str = "test::__rust_begin_short_backtrace";
const BACKTRACE_NOTE: &str = "note: run with `RUST_BACKTRACE=1`";

fn kept() -> Arc<Mutex<Vec<u8>>> {
    Arc::default()
}

impl Host {
    pub fn run_tests(
        &self,
        wasm: &Path,
        arguments: &[String],
        directories: &[PathBuf],
        libraries: &[PathBuf],
    ) -> Result<i32, String> {
        let tests = Tests {
            module: Stored::file(wasm.to_path_buf())?.into_bytes()?,
            libraries: libraries.to_vec(),
        };
        let stderr = Output::Tee {
            kept: kept(),
            held: false,
        };
        match self.run_instance(&tests, arguments, directories, Output::Inherited, stderr) {
            Ok(0) => return Ok(0),
            Ok(_) => {}
            Err(failure) => eprintln!("{failure}"),
        }
        let names = self.list_tests(&tests, arguments, directories)?;
        eprintln!(
            "a panic ends a wasm test run, so the {} tests run again one at a time",
            names.len()
        );
        let carried: Vec<&String> = arguments
            .iter()
            .skip(1)
            .filter(|argument| CARRIED.contains(&argument.as_str()))
            .collect();
        let mut failed = Vec::new();
        for name in names {
            let mut single = vec![arguments[0].clone(), name.clone(), "--exact".to_owned()];
            single.extend(carried.iter().map(|argument| (*argument).clone()));
            single.extend(["--test-threads=1".to_owned(), "--nocapture".to_owned()]);
            let stdout = Output::Kept(kept());
            let stderr = Output::Tee {
                kept: kept(),
                held: true,
            };
            let failure = match self.run_instance(
                &tests,
                &single,
                directories,
                stdout.clone(),
                stderr.clone(),
            ) {
                Ok(0) => continue,
                Ok(code) => format!("exit {code}"),
                Err(failure) => failure,
            };
            print!("{}", stdout.contents());
            eprint!("{}", stderr.contents());
            eprintln!("{failure}");
            failed.push((name, why(&stderr, &failure)));
        }
        match failed.is_empty() {
            true => eprintln!("every test passed on its own, but not in one run together"),
            false => {
                eprintln!("{} tests failed:", failed.len());
                for (name, why) in &failed {
                    eprintln!("    {name}");
                    for line in why.lines() {
                        eprintln!("        {line}");
                    }
                }
            }
        }
        Ok(1)
    }

    fn list_tests(
        &self,
        tests: &Tests,
        arguments: &[String],
        directories: &[PathBuf],
    ) -> Result<Vec<String>, String> {
        let mut listing: Vec<String> = arguments
            .iter()
            .filter(|argument| {
                !argument.starts_with("--test-threads") && argument.as_str() != "--nocapture"
            })
            .cloned()
            .collect();
        listing.extend(["--list", "--format", "terse"].map(str::to_owned));
        let output = Output::Kept(kept());
        let stderr = Output::Tee {
            kept: kept(),
            held: false,
        };
        let code = self.run_instance(tests, &listing, directories, output.clone(), stderr)?;
        if code != 0 {
            return Err(format!("the tests could not be listed: exit {code}"));
        }
        Ok(output
            .contents()
            .lines()
            .filter_map(|line| line.strip_suffix(": test"))
            .map(str::to_owned)
            .collect())
    }

    fn run_instance(
        &self,
        tests: &Tests,
        arguments: &[String],
        directories: &[PathBuf],
        stdout: Output,
        stderr: Output,
    ) -> Result<i32, String> {
        let wasi = Wasi::run(arguments, stdout, stderr, directories)?;
        let identities = crate::dylink::libraries(&crate::engine::Shape::read(&tests.module)?)?;
        if identities.len() != tests.libraries.len() {
            return Err(format!(
                "the tests are linked against {} libraries, and {} were given",
                identities.len(),
                tests.libraries.len()
            ));
        }
        let library = |identity: &str| {
            let index = identities
                .iter()
                .position(|linked| linked == identity)
                .ok_or_else(|| format!("the tests are not linked against {identity}"))?;
            let path = &tests.libraries[index];
            Stored::file(path.clone())?
                .into_bytes()
                .map_err(|error| format!("{} could not be loaded: {error}", path.display()))
        };
        let wake: Arc<Wake> = Arc::default();
        let setup = Setup {
            wasi,
            initialize: false,
        };
        let mut running = Running::new(self, tests.module.clone(), &library, &wake, setup)
            .map_err(|error| format!("the tests could not be instantiated: {error}"))?;
        let entry = match identities.is_empty() {
            true => START,
            false => MAIN,
        };
        let (outcome, _) = running.call(entry);
        let state = running.state_mut();
        if let Some(failure) = state.take_error().or_else(|| state.threads.take_failure()) {
            return Err(failure);
        }
        if let Some(code) = state
            .fatal
            .as_deref()
            .and_then(|fatal| fatal.strip_prefix(EXITED))
            .and_then(|code| code.parse().ok())
        {
            return Ok(code);
        }
        match outcome {
            Ok(code) => Ok(code.unwrap_or(0)),
            Err(_) if state.fatal.is_some() => Err(state.fatal.take().unwrap_or_default()),
            Err(thrown) if aborted(&thrown) => Err(aborted_with_backtrace(&thrown)),
            Err(thrown) => Err(format!("the tests trapped: {thrown}")),
        }
    }
}

/// The tests, as one module with everything in it whose `_start` runs them, or linked against
/// `libraries` (guides/app_libraries.md), whose `__main_void` runs them once they are linked as
/// an app's are. The libraries are given in the order the module lists them, its whole closure
/// (buck/wasm/defs.bzl passes them so); each is checked against the identity it is given for.
struct Tests {
    module: Vec<u8>,
    libraries: Vec<PathBuf>,
}

/// The names of the functions on a trap's stack as V8 writes it (`at MODULE.FUNCTION (wasm://…)`),
/// innermost first, demangled.
fn frames(thrown: &str) -> Vec<String> {
    thrown
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("at "))
        .map(|frame| {
            let name = match frame.find(" (") {
                Some(end) => &frame[..end],
                None => frame,
            };
            let function = ["._ZN", "._R"]
                .iter()
                .filter_map(|mangled| name.find(mangled).map(|at| &name[at + 1..]))
                .next()
                .unwrap_or_else(|| name.rsplit('.').next().unwrap_or(name));
            format!("{:#}", rustc_demangle::demangle(function))
        })
        .collect()
}

fn aborted(thrown: &str) -> bool {
    thrown.starts_with(UNREACHABLE) && frames(thrown).first().map(String::as_str) == Some(ABORT)
}

fn aborted_with_backtrace(thrown: &str) -> String {
    let names = frames(thrown);
    let end = names
        .iter()
        .position(|name| name.starts_with(TEST_HARNESS))
        .unwrap_or(names.len());
    let start = names[..end]
        .iter()
        .rposition(|name| {
            PANIC_MACHINERY
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .map_or(0, |last| last + 1);
    let mut message = "the tests aborted, at:".to_owned();
    for name in &names[start..end] {
        message.push_str("\n    ");
        message.push_str(name);
    }
    message
}

fn why(stderr: &Output, failure: &str) -> String {
    let written = stderr.contents();
    let panic = written
        .lines()
        .position(|line| line.contains(PANICKED))
        .map(|start| {
            written
                .lines()
                .skip(start)
                .filter(|line| !line.starts_with(BACKTRACE_NOTE))
                .collect::<Vec<_>>()
                .join("\n")
        });
    match panic {
        Some(panic) => format!("{panic}\n{failure}"),
        None => format!("{}\n{failure}", written.trim_end())
            .trim_start()
            .to_owned(),
    }
}
