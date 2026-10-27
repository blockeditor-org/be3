use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: verify verify MANIFEST [--build-failed] [--check] [--lint] [--tests] [--plugin-tests] [--previews BASE OUT] [--bootstrap DIR]";

struct Options {
    manifest: PathBuf,
    build_failed: bool,
    check: bool,
    lint: bool,
    plugin_tests: bool,
    previews: Option<(String, PathBuf)>,
    bootstrap: Option<PathBuf>,
}

struct Run {
    options: Options,
    built: Vec<(String, PathBuf, PathBuf, bool)>,
    failed: bool,
}

pub fn run() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().is_some_and(|first| first == "verify") {
        arguments.remove(0);
    }
    main(arguments)
}

fn main(arguments: Vec<String>) -> ExitCode {
    let options = match parse(arguments.into_iter()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let manifest = match fs::read_to_string(&options.manifest) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("could not read {}: {error}", options.manifest.display());
            println!("\nVerification failed.");
            return ExitCode::FAILURE;
        }
    };
    let built = manifest
        .lines()
        .filter_map(|line| line.rsplit_once('\t'))
        .map(|(kind, path)| {
            let (kind, package) = kind.split_once('\t').unwrap_or((kind, ""));
            let path = PathBuf::from(path);
            let exists = path.exists();
            (kind.to_owned(), PathBuf::from(package), path, exists)
        })
        .collect();
    let mut run = Run {
        failed: options.build_failed,
        options,
        built,
    };
    run.verify();
    println!();
    if run.failed {
        if run.options.build_failed {
            println!(
                "Verification failed: something did not build or a test failed, and buck2 says what above."
            );
        } else {
            println!("Verification failed.");
        }
        return ExitCode::FAILURE;
    }
    println!("All checks passed.");
    ExitCode::SUCCESS
}

fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let manifest = arguments.next().ok_or("no manifest")?.into();
    let mut options = Options {
        manifest,
        build_failed: false,
        check: false,
        lint: false,
        plugin_tests: false,
        previews: None,
        bootstrap: None,
    };
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--build-failed" => options.build_failed = true,
            "--check" => options.check = true,
            "--lint" => options.lint = true,
            "--tests" => {}
            "--plugin-tests" => options.plugin_tests = true,
            "--bootstrap" => options.bootstrap = Some(value()?.into()),
            "--previews" => {
                let base = value()?;
                options.previews = Some((base, value()?.into()));
            }
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    Ok(options)
}

impl Run {
    fn verify(&mut self) {
        if self.options.lint {
            self.executable_bits();
        }
        self.generated();
        if self.options.lint {
            self.fixes();
        }
        self.bootstrap();
        if self.options.plugin_tests {
            self.paintings();
            self.unpainted();
        }
        if self.options.previews.is_some() {
            self.local_tools();
        }
    }

    fn local_tools(&mut self) {
        let Some(tool) = self.built("local-tools").into_iter().next() else {
            println!("crates/local-tools did not build, so the paint previews were not made.");
            self.failed = true;
            return;
        };
        let mut command = Command::new(tool);
        command.arg("ci").arg(&self.options.manifest);
        if let Some((base, out)) = &self.options.previews {
            command.arg("--previews").arg(base).arg(out);
        }
        if !command.status().is_ok_and(|status| status.success()) {
            self.failed = true;
        }
    }

    fn built(&self, kind: &str) -> Vec<PathBuf> {
        self.built_in(kind)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }

    fn built_in(&self, kind: &str) -> Vec<(PathBuf, PathBuf)> {
        self.built
            .iter()
            .filter(|(each, _, _, exists)| each == kind && *exists)
            .map(|(_, package, path, _)| (package.clone(), path.clone()))
            .collect()
    }

    fn expected(&self, kind: &str) -> usize {
        self.built
            .iter()
            .filter(|(each, _, _, _)| each == kind)
            .count()
    }

    fn executable_bits(&mut self) {
        let Some(listed) = git(&["ls-files", "-z"]) else {
            self.failed = true;
            return;
        };
        let executable: Vec<&str> = listed
            .split('\0')
            .filter(|path| !path.is_empty() && is_executable(Path::new(path)))
            .filter(|path| {
                let script = path
                    .strip_prefix("scripts/")
                    .is_some_and(|name| !name.contains('/'));
                !script
            })
            .collect();
        if executable.is_empty() {
            return;
        }
        if self.options.check {
            println!("These files are executable; run ./scripts/verify without --check:");
            for path in executable {
                println!("  {path}");
            }
            self.failed = true;
            return;
        }
        for path in executable {
            if let Err(error) = clear_executable(Path::new(path)) {
                println!("Could not make {path} not executable: {error}");
                self.failed = true;
            }
        }
    }

    fn generated(&mut self) {
        if let Some(generated) = self.built("buckify").first() {
            self.update(
                &generated.join("crates.bzl"),
                Path::new("buck/cargo/crates.bzl"),
            );
            self.update(&generated.join("Cargo.lock"), Path::new("Cargo.lock"));
        }
        if let Some(lock) = self.built("sysroot").first() {
            self.update(lock, Path::new("buck/sysroot/packages.bzl"));
        }
    }

    fn bootstrap(&mut self) {
        let built = self.built("bootstrap").first().cloned();
        match bootstrap::check(
            built.as_deref(),
            self.options.bootstrap.as_deref(),
            self.options.check,
        ) {
            Ok(bootstrap::Outcome::Current) => {}
            Ok(bootstrap::Outcome::Published(lines)) => {
                println!(
                    "crates/bootstrap changed, so its new release is pinned in scripts/internal/pins:"
                );
                for line in lines {
                    println!("  {line}");
                }
            }
            Ok(bootstrap::Outcome::Stale) => {
                println!(
                    "crates/bootstrap built from this checkout is not the release scripts/internal/pins names; \
                     CI on a pull request uploads and pins it (guides/bootstrap.md)."
                );
                self.failed |= self.options.check;
            }
            Err(message) => {
                println!("Could not pin crates/bootstrap's release: {message}");
                self.failed = true;
            }
        }
    }

    fn update(&mut self, from: &Path, to: &Path) {
        if fs::read(from).ok() == fs::read(to).ok() {
            return;
        }
        if self.options.check {
            println!("Out of date: {}", to.display());
            self.failed = true;
            return;
        }
        let mut partial = to.as_os_str().to_owned();
        partial.push(".partial");
        let partial = PathBuf::from(partial);
        let written = create_parent(to)
            .and_then(|()| fs::copy(from, &partial))
            .and_then(|_| fs::rename(&partial, to));
        if written.is_ok() {
            println!("Updated {}", to.display());
        } else {
            println!("Could not update {}", to.display());
            self.failed = true;
        }
    }

    fn fixes(&mut self) {
        let fixes = self.built("fix");
        let mut changed = Vec::new();
        let mut deleted = BTreeSet::new();
        let mut originals = BTreeSet::new();
        let mut stale = BTreeSet::new();
        let mut added = BTreeSet::new();
        for fix in &fixes {
            let root = fix.join("changed");
            let original = fix.join("original");
            for path in files_under(&root) {
                if read_the_same(&original.join(&path), Path::new(&path)) {
                    if !original.join(&path).exists() {
                        added.insert(path.clone());
                    }
                    changed.push((root.join(&path), path));
                    originals.insert(original.clone());
                } else {
                    stale.insert(path);
                }
            }
            if let Ok(listed) = fs::read_to_string(fix.join("deleted")) {
                for path in listed.lines().filter(|line| !line.is_empty()) {
                    let gone = !Path::new(path).exists();
                    if gone || read_the_same(&original.join(path), Path::new(path)) {
                        deleted.insert(path.to_owned());
                        originals.insert(original.clone());
                    } else {
                        stale.insert(path.to_owned());
                    }
                }
            }
        }
        if !stale.is_empty() {
            println!(
                "These files changed after the fixes read them, so their fixes were left out; run ./scripts/verify again:"
            );
            for path in &stale {
                println!("  {path}");
            }
            self.failed = true;
        }
        if !changed.is_empty() || !deleted.is_empty() {
            if self.options.check {
                println!(
                    "The autofixes would change these files; run ./scripts/verify without --check:"
                );
                let paths: BTreeSet<&str> = changed.iter().map(|(_, path)| path.as_str()).collect();
                for path in paths.into_iter().chain(deleted.iter().map(String::as_str)) {
                    println!("  {path}");
                }
                self.failed = true;
            } else {
                println!("Fixed:");
                for (from, path) in &changed {
                    let to = Path::new(path);
                    if create_parent(to).and_then(|()| fs::copy(from, to)).is_ok() {
                        println!("  {path}");
                    } else {
                        println!("  {path} could not be written");
                        self.failed = true;
                    }
                }
                for path in &deleted {
                    match fs::remove_file(path) {
                        Ok(()) => println!("  {path} (deleted)"),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            println!("  {path} (deleted)");
                        }
                        Err(_) => {
                            println!("  {path} could not be deleted");
                            self.failed = true;
                        }
                    }
                }
                println!(
                    "Until the next ./scripts/verify, the files as they were before these fixes are under {}",
                    one_pattern(&originals)
                );
                if !added.is_empty() || !deleted.is_empty() {
                    println!(
                        "These fixes moved code between files, and the build and the tests ran on the files as they were before, so they are unchecked; run ./scripts/verify again to check them."
                    );
                    self.failed = true;
                }
            }
        }
    }

    fn paintings(&mut self) {
        let expected = self.expected("paintings");
        let directories = self.built_in("paintings");
        let mut changed = Vec::new();
        let mut stale = Vec::new();
        for (package, directory) in &directories {
            for painting in paint_files(&directory.join("changed")) {
                let name = painting.file_name().unwrap_or_default();
                let compared =
                    fs::read_to_string(directory.join("used").join(name)).unwrap_or_default();
                let to = package.join("snapshots").join(name);
                let accepted = fs::read(&to).unwrap_or_default();
                if compared.trim() == bootstrap::sha256(&accepted) {
                    changed.push((painting, to));
                } else {
                    stale.push(to);
                }
            }
        }
        if !stale.is_empty() {
            println!(
                "These paintings changed after their tests compared them, so they were left as they are; run ./scripts/verify again:"
            );
            for painting in &stale {
                println!("  {}", slashed(painting));
            }
            self.failed = true;
        }
        if !changed.is_empty() {
            if self.options.check {
                println!(
                    "These paintings changed; run ./scripts/verify without --check to accept them, then review them on the pull request:"
                );
            } else {
                println!("Accepting the paintings that changed:");
            }
            for (painting, to) in &changed {
                let mut why = painting.as_os_str().to_owned();
                why.push(".why");
                let why = fs::read_to_string(PathBuf::from(why)).unwrap_or_default();
                println!("  {}: {}", slashed(to), why.trim_end_matches('\n'));
                if !self.options.check
                    && (create_parent(to).is_err() || fs::copy(painting, to).is_err())
                {
                    println!("  {} could not be written", slashed(to));
                    self.failed = true;
                }
            }
            self.failed |= self.options.check;
        }
        if expected == 0 || directories.len() != expected || self.options.build_failed {
            return;
        }
        let used: BTreeSet<PathBuf> = directories
            .iter()
            .flat_map(|(package, directory)| {
                paint_files(&directory.join("used"))
                    .into_iter()
                    .filter_map(|painting| {
                        Some(package.join("snapshots").join(painting.file_name()?))
                    })
            })
            .collect();
        let unused: Vec<PathBuf> = accepted_paintings(Path::new("crates"))
            .into_iter()
            .filter(|painting| !used.contains(painting))
            .collect();
        if unused.is_empty() {
            return;
        }
        if self.options.check {
            println!(
                "No test compared these paintings; run ./scripts/verify without --check to delete them:"
            );
        } else {
            println!("Deleting the paintings no test compared:");
        }
        for painting in &unused {
            println!("  {}", slashed(painting));
        }
        if self.options.check {
            self.failed = true;
            return;
        }
        for painting in &unused {
            if fs::remove_file(painting).is_err() {
                println!("  {} could not be deleted", slashed(painting));
                self.failed = true;
            }
        }
    }

    fn unpainted(&mut self) {
        let missing: Vec<PathBuf> = named_paintings(Path::new("crates"))
            .into_iter()
            .filter(|painting| !painting.exists())
            .collect();
        if missing.is_empty() {
            return;
        }
        if self.options.build_failed {
            println!(
                "A test names each of these paintings but none was written, most likely because the test failed before it painted (its failure is above):"
            );
        } else {
            println!(
                "A test names each of these paintings but none was written: the test passed without reaching its snapshot:"
            );
        }
        for painting in &missing {
            println!("  {}", slashed(painting));
        }
        self.failed = true;
    }
}

fn git(arguments: &[&str]) -> Option<String> {
    git_bytes(arguments).and_then(|bytes| String::from_utf8(bytes).ok())
}

fn git_bytes(arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(arguments)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn named_paintings(crates: &Path) -> BTreeSet<PathBuf> {
    let mut names = BTreeSet::new();
    for directory in directories_under(crates) {
        let paints = fs::read_to_string(directory.join("BUCK")).is_ok_and(|build| {
            build.lines().any(|line| {
                line.starts_with("editor(")
                    || line.starts_with("plugin_tests(")
                    || line.trim() == "paints = True,"
            })
        });
        if !paints {
            continue;
        }
        for file in files_under(&directory.join("src")) {
            if !file.ends_with(".rs") {
                continue;
            }
            let Ok(source) = fs::read_to_string(directory.join("src").join(&file)) else {
                continue;
            };
            for painting in literal_snapshots(&source) {
                names.insert(
                    directory
                        .join("snapshots")
                        .join(format!("{painting}.paint")),
                );
            }
        }
    }
    names
}

fn accepted_paintings(crates: &Path) -> Vec<PathBuf> {
    directories_under(crates)
        .into_iter()
        .filter(|directory| directory.join("Cargo.toml").exists())
        .flat_map(|directory| paint_files(&directory.join("snapshots")))
        .collect()
}

fn directories_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir())
                && !entry.file_name().to_string_lossy().starts_with('.')
            {
                pending.push(entry.path());
            }
        }
        found.push(directory);
    }
    found
}

fn literal_snapshots(source: &str) -> Vec<&str> {
    source
        .split(".snapshot(")
        .skip(1)
        .filter_map(|rest| {
            let rest = rest.trim_start().strip_prefix('"')?;
            let name = &rest[..rest.find('"')?];
            let plain = !name.is_empty()
                && name.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "_-.".contains(character)
                });
            plain.then_some(name)
        })
        .collect()
}

fn read_the_same(original: &Path, path: &Path) -> bool {
    fs::read(original).ok() == fs::read(path).ok()
}

fn create_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => fs::create_dir_all(parent),
        _ => Ok(()),
    }
}

fn files_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let Ok(entries) = fs::read_dir(root.join(&relative)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = relative.join(entry.file_name());
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(path),
                Ok(_) => found.push(slashed(&path)),
                Err(_) => {}
            }
        }
    }
    found.sort();
    found
}

fn paint_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "paint")
        })
        .collect();
    found.sort();
    found
}

fn one_pattern(paths: &BTreeSet<PathBuf>) -> String {
    let split: Vec<Vec<_>> = paths
        .iter()
        .map(|path| path.components().collect())
        .collect();
    let Some(first) = split.first() else {
        return String::new();
    };
    if split.len() == 1 {
        return first.iter().collect::<PathBuf>().display().to_string();
    }
    let shortest = split.iter().map(Vec::len).min().unwrap_or(0);
    let shared = |at: &dyn Fn(&Vec<_>) -> _| split.iter().all(|path| at(path) == at(first));
    let prefix = (0..shortest)
        .take_while(|&i| shared(&|path: &Vec<_>| path[i]))
        .count();
    let suffix = (0..shortest - prefix)
        .take_while(|&i| shared(&|path: &Vec<_>| path[path.len() - 1 - i]))
        .count();
    let mut pattern: PathBuf = first[..prefix].iter().collect();
    pattern.push("*");
    pattern.extend(&first[first.len() - suffix..]);
    pattern.display().to_string()
}

fn slashed(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_: &Path) -> bool {
    false
}

#[cfg(unix)]
fn clear_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() & !0o111);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn clear_executable(_: &Path) -> io::Result<()> {
    Ok(())
}

mod bootstrap;

#[cfg(test)]
mod tests;
