use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::git;
use crate::previews::{PAINTINGS, stage};

const USAGE: &str = "usage: local-tools ci MANIFEST [--previews BASE OUT] [--unchecked PACKAGE]... [--unchecked-all]";

struct Options {
    previews: Option<(String, PathBuf)>,
    /// The packages whose reports of a second run may be an earlier build's,
    /// since something failed (be-tool-verify's failures.rs), and whether that
    /// is every package.
    unchecked: Vec<String>,
    unchecked_all: bool,
}

struct Run {
    options: Options,
    /// Each line of the manifest: its kind, its package (empty but for a
    /// test's paintings and their second run), its path and whether it exists.
    built: Vec<(String, String, PathBuf, bool)>,
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
        .filter_map(|line| line.rsplit_once('\t'))
        .map(|(kind, path)| {
            let (kind, package) = kind.split_once('\t').unwrap_or((kind, ""));
            let path = PathBuf::from(path);
            let exists = path.exists();
            (kind.to_owned(), package.to_owned(), path, exists)
        })
        .collect();
    let mut run = Run {
        failed: false,
        options,
        built,
    };
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
        previews: None,
        unchecked: Vec::new(),
        unchecked_all: false,
    };
    while let Some(argument) = arguments.next() {
        let mut value = || arguments.next().ok_or(format!("{argument} needs a value"));
        match argument.as_str() {
            "--unchecked" => options.unchecked.push(value()?),
            "--unchecked-all" => options.unchecked_all = true,
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
        self.built_in(kind)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }

    fn built_in(&self, kind: &str) -> Vec<(String, PathBuf)> {
        self.built
            .iter()
            .filter(|(each, _, _, exists)| each == kind && *exists)
            .map(|(_, package, path, _)| (package.clone(), path.clone()))
            .collect()
    }

    /// The comment's section on the tests whose second run disagreed with
    /// their first (_again in buck/dev/test.bzl), empty when none did, and
    /// what was not looked at because something failed.
    fn flaky(&self) -> String {
        if self.options.unchecked_all {
            return "## Flaky paintings\n\nNot looked for: something besides a test failed to build, so a report may be an earlier build's.\n\n".to_owned();
        }
        let mut section = String::new();
        for (package, directory) in self.built_in("flaky") {
            if self.options.unchecked.contains(&package) {
                continue;
            }
            let flaky = fs::read_to_string(directory.join("flaky")).unwrap_or_default();
            for line in flaky.lines() {
                section.push_str(&format!("- `{package}`: {line}\n"));
            }
        }
        if !self.options.unchecked.is_empty() {
            section.push_str(&format!(
                "- Not looked for in {}, whose tests failed.\n",
                self.options
                    .unchecked
                    .iter()
                    .map(|package| format!("`{package}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if section.is_empty() {
            return section;
        }
        format!(
            "## Flaky paintings\n\nThese tests painted differently, or failed, when CI ran them once more, so the paintings this pull request accepted from them may flip:\n\n{section}\n"
        )
    }

    fn previews(&self, base: &str, out: &Path) -> Result<(), String> {
        let from = git(&["merge-base", base, "HEAD"]).ok_or("no merge base")?;
        let from = from.trim();
        let changed = git(&[
            "diff",
            "--name-only",
            "--no-renames",
            from,
            "HEAD",
            "--",
            PAINTINGS,
        ])
        .ok_or("could not list the paintings that changed")?;
        let (before, after) = stage(changed.lines(), from, Some("HEAD"))?;
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
        let mut comment = self.flaky();
        comment.push_str("## Lines of code\n\n");
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

mod lines;
mod unsafe_counts;

#[cfg(test)]
mod tests;
