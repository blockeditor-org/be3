use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str =
    "usage: local-tools ci MANIFEST [--build-failed] [--android DIR] [--previews BASE OUT]";

struct Options {
    build_failed: bool,
    android: Option<PathBuf>,
    previews: Option<(String, PathBuf)>,
}

struct Run {
    options: Options,
    built: Vec<(String, PathBuf, bool)>,
    failed: bool,
}

pub fn main(arguments: Vec<String>) -> ExitCode {
    let mut arguments = arguments.into_iter();
    let Some(manifest) = arguments.next() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let options = match parse(arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let manifest = match fs::read_to_string(&manifest) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("could not read {manifest}: {error}");
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
        failed: false,
        options,
        built,
    };
    if let Some(directory) = run.options.android.clone() {
        run.android(&directory);
    }
    if let Some((base, out)) = run.options.previews.clone()
        && let Err(message) = run.previews(&base, &out)
    {
        println!("Could not render the paint previews: {message}");
        run.failed = true;
    }
    if run.failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn parse(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        build_failed: false,
        android: None,
        previews: None,
    };
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--build-failed" => options.build_failed = true,
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
    fn built(&self, kind: &str) -> Vec<PathBuf> {
        self.built
            .iter()
            .filter(|(each, _, exists)| each == kind && *exists)
            .map(|(_, path, _)| path.clone())
            .collect()
    }

    fn android(&mut self, directory: &Path) {
        if fs::create_dir_all(directory).is_err() {
            self.failed = true;
            return;
        }
        for apk in ["block-app.apk", "be-launcher.apk"] {
            let output = directory.join(apk);
            if let Err(error) = fs::remove_file(&output)
                && error.kind() != io::ErrorKind::NotFound
            {
                eprintln!("{}: {error}", output.display());
                self.failed = true;
                return;
            }
        }
        if self.options.build_failed {
            return;
        }
        let finalize = || -> Result<(), String> {
            let metadata = self
                .built("apk-metadata")
                .into_iter()
                .next()
                .ok_or("no APK metadata was built")?;
            let key = crate::apk_sign::ci_key();
            for apk in ["block-app.apk", "be-launcher.apk"] {
                let template = self
                    .built(apk)
                    .into_iter()
                    .next()
                    .ok_or_else(|| format!("no {apk} template was built"))?;
                crate::apk_sign::finalize(&template, &directory.join(apk), &key, &metadata)?;
            }
            Ok(())
        };
        if let Err(error) = finalize() {
            eprintln!("Could not finalize Android APKs: {error}");
            for apk in ["block-app.apk", "be-launcher.apk"] {
                let _ = fs::remove_file(directory.join(apk));
            }
            self.failed = true;
        }
    }

    fn previews(&self, base: &str, out: &Path) -> Result<(), String> {
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
        let numstat = git(&["diff", "--numstat", "--no-renames", from, "HEAD"])
            .ok_or("could not count the lines that changed")?;
        let added = git(&[
            "diff",
            "--name-only",
            "--no-renames",
            "--diff-filter=A",
            from,
            "HEAD",
        ])
        .ok_or("could not list the files that were added")?;
        let added: Vec<&str> = added.lines().collect();
        let mut comment = "## Lines of code\n\n".to_owned();
        comment.push_str(&lines::summary(&numstat, &added));
        comment.push_str(&unsafe_counts::summary(
            &unsafe_at(from)?,
            &unsafe_at("HEAD")?,
        ));
        let renderers = self.built("browser-rasterizer");
        let site = self.built("paint-review-site");
        comment.push_str(&crate::previews::generate(
            &before,
            &after,
            out,
            renderers.first().map(PathBuf::as_path),
            site.first().map(PathBuf::as_path),
        )?);
        let path = out.join("comment.md");
        fs::write(path, comment).map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn unsafe_at(commit: &str) -> Result<unsafe_counts::Counts, String> {
    let listed = git(&[
        "grep",
        "-l",
        "-w",
        "unsafe",
        commit,
        "--",
        ":(glob)crates/**/*.rs",
    ])
    .unwrap_or_default();
    let mut sources = Vec::new();
    for line in listed.lines() {
        let source = git(&["show", line])
            .ok_or_else(|| format!("could not read {line} to count its unsafe sites"))?;
        sources.push(source);
    }
    Ok(unsafe_counts::count(sources.iter().map(String::as_str)))
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

mod lines;
mod unsafe_counts;

#[cfg(test)]
mod tests;
