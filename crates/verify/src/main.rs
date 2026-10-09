use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: verify MANIFEST [--build-failed] [--check] [--lint] [--tests] [--plugin-tests] [--android DIR] [--previews BASE OUT]";

const EXECUTABLE: &[&str] = &[
    "scripts/internal/install-buck2.sh",
    "scripts/internal/install-nsc.sh",
];

struct Options {
    manifest: PathBuf,
    build_failed: bool,
    check: bool,
    lint: bool,
    plugin_tests: bool,
    android: Option<PathBuf>,
    previews: Option<(String, PathBuf)>,
}

struct Run {
    options: Options,
    built: Vec<(String, PathBuf, bool)>,
    failed: bool,
}

fn main() -> ExitCode {
    let options = match parse(std::env::args().skip(1)) {
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
        .filter_map(|line| line.split_once('\t'))
        .map(|(kind, path)| {
            let path = PathBuf::from(path);
            let exists = path.exists();
            (kind.to_owned(), path, exists)
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
        println!("Verification failed.");
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
        android: None,
        previews: None,
    };
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--build-failed" => options.build_failed = true,
            "--check" => options.check = true,
            "--lint" => options.lint = true,
            "--tests" => {}
            "--plugin-tests" => options.plugin_tests = true,
            "--android" => options.android = Some(value()?.into()),
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
        if self.options.plugin_tests {
            self.paintings();
        }
        if let Some(directory) = self.options.android.clone() {
            self.android(&directory);
        }
        if let Some((base, out)) = self.options.previews.clone()
            && let Err(message) = self.previews(&base, &out)
        {
            println!("Could not render the paint previews: {message}");
            self.failed = true;
        }
    }

    fn built(&self, kind: &str) -> Vec<PathBuf> {
        self.built
            .iter()
            .filter(|(each, _, exists)| each == kind && *exists)
            .map(|(_, path, _)| path.clone())
            .collect()
    }

    fn expected(&self, kind: &str) -> usize {
        self.built
            .iter()
            .filter(|(each, _, _)| each == kind)
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
                !script && !EXECUTABLE.contains(path)
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
        let mut findings = BTreeMap::new();
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
            if let Ok(entries) = fs::read_dir(fix.join("findings")) {
                for entry in entries.flatten() {
                    findings
                        .entry(entry.file_name())
                        .or_insert_with(|| entry.path());
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
                    "Until the next ./scripts/verify, the files as they were before these fixes are under:"
                );
                for original in &originals {
                    println!("  {}", original.display());
                }
                if !added.is_empty() || !deleted.is_empty() {
                    println!(
                        "These fixes moved code between files, and the build and the tests ran on the files as they were before, so they are unchecked; run ./scripts/verify again to check them."
                    );
                    self.failed = true;
                }
            }
        }
        if !findings.is_empty() {
            let mut stdout = io::stdout().lock();
            for path in findings.values() {
                if let Ok(text) = fs::read(path) {
                    let _ = stdout.write_all(&text);
                }
            }
            drop(stdout);
            println!("clippy: {} findings.", findings.len());
            self.failed = true;
        }
    }

    fn paintings(&mut self) {
        let expected = self.expected("paintings");
        let directories = self.built("paintings");
        let mut changed = Vec::new();
        let mut stale = Vec::new();
        for directory in &directories {
            for painting in paint_files(&directory.join("changed")) {
                let name = painting.file_name().unwrap_or_default();
                let compared = fs::read(directory.join("used").join(name)).unwrap_or_default();
                let accepted = fs::read(Path::new("snapshots").join(name)).unwrap_or_default();
                if compared == accepted {
                    changed.push((painting, directory.join("used")));
                } else {
                    stale.push(painting);
                }
            }
        }
        if !stale.is_empty() {
            println!(
                "These paintings changed after their tests compared them, so they were left as they are; run ./scripts/verify again:"
            );
            for painting in &stale {
                let name = painting.file_name().unwrap_or_default().to_string_lossy();
                println!("  snapshots/{name}");
            }
            self.failed = true;
        }
        if !changed.is_empty() {
            if self.options.check {
                println!(
                    "These paintings changed; run ./scripts/verify without --check to accept them, then review them in a Paint review block:"
                );
            } else {
                println!("Accepting the paintings that changed:");
            }
            for (painting, _) in &changed {
                let name = painting.file_name().unwrap_or_default().to_string_lossy();
                let mut why = painting.as_os_str().to_owned();
                why.push(".why");
                let why = fs::read_to_string(PathBuf::from(why)).unwrap_or_default();
                println!("  snapshots/{name}: {}", why.trim_end_matches('\n'));
                if !self.options.check
                    && fs::copy(painting, Path::new("snapshots").join(&*name)).is_err()
                {
                    println!("  snapshots/{name} could not be written");
                    self.failed = true;
                }
            }
            if self.options.check {
                self.failed = true;
            } else {
                println!(
                    "Until the next ./scripts/verify, the paintings as they were before are under these, empty where there was none:"
                );
                let previous: BTreeSet<&PathBuf> = changed.iter().map(|(_, used)| used).collect();
                for used in previous {
                    println!("  {}", used.display());
                }
            }
        }
        if expected == 0 || directories.len() != expected || self.options.build_failed {
            return;
        }
        let used: BTreeSet<_> = directories
            .iter()
            .flat_map(|directory| paint_files(&directory.join("used")))
            .filter_map(|painting| painting.file_name().map(ToOwned::to_owned))
            .collect();
        let unused: Vec<PathBuf> = paint_files(Path::new("snapshots"))
            .into_iter()
            .filter(|painting| {
                painting
                    .file_name()
                    .is_some_and(|name| !used.contains(name))
            })
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

    fn android(&mut self, directory: &Path) {
        if fs::create_dir_all(directory).is_err() {
            self.failed = true;
            return;
        }
        for apk in ["block-app.apk", "be-launcher.apk"] {
            let copied = self
                .built(apk)
                .first()
                .is_some_and(|path| fs::copy(path, directory.join(apk)).is_ok());
            if !copied {
                self.failed = true;
            }
        }
    }

    fn previews(&self, base: &str, out: &Path) -> Result<(), String> {
        let renderer = self
            .built("previews")
            .into_iter()
            .next()
            .ok_or("the renderer did not build")?;
        let from = git(&["merge-base", base, "HEAD"]).ok_or("no merge base")?;
        let from = from.trim();
        let previews = Path::new("target/previews");
        let (before, after) = (previews.join("before"), previews.join("after"));
        for directory in [&before, &after] {
            let _ = fs::remove_dir_all(directory);
            fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        let changed = git(&[
            "diff",
            "--name-only",
            "--no-renames",
            from,
            "HEAD",
            "--",
            "snapshots/*.paint",
        ])
        .ok_or("could not list the paintings that changed")?;
        for path in changed.lines().filter(|line| !line.is_empty()) {
            let name = Path::new(path)
                .file_name()
                .ok_or("a painting with no name")?;
            for (commit, directory) in [(from, &before), ("HEAD", &after)] {
                if let Some(contents) = git_bytes(&["show", &format!("{commit}:{path}")]) {
                    fs::write(directory.join(name), contents).map_err(|error| error.to_string())?;
                }
            }
        }
        let numstat = git(&[
            "diff",
            "--numstat",
            from,
            "HEAD",
            "--",
            ".",
            ":(exclude,glob)**/tests.rs",
            ":(exclude,glob)**/tests/**",
        ])
        .ok_or("could not count the lines that changed")?;
        let (mut added, mut removed, mut files) = (0u64, 0u64, 0u64);
        for line in numstat.lines() {
            let mut fields = line.split('\t');
            if let (Some(plus), Some(minus)) = (fields.next(), fields.next())
                && let (Ok(plus), Ok(minus)) = (plus.parse::<u64>(), minus.parse::<u64>())
            {
                added += plus;
                removed += minus;
                files += 1;
            }
        }
        let status = Command::new(&renderer)
            .arg(&before)
            .arg(&after)
            .arg(out)
            .args([added.to_string(), removed.to_string(), files.to_string()])
            .status()
            .map_err(|error| format!("{}: {error}", renderer.display()))?;
        if !status.success() {
            return Err(format!("the renderer exited with {status}"));
        }
        Ok(())
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
