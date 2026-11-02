use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{archive, tls};

struct Input {
    at: usize,
    tokens: usize,
    path: PathBuf,
}

fn has_extension(argument: &str) -> bool {
    [".o", ".rlib", ".a"]
        .iter()
        .any(|extension| argument.ends_with(extension))
}

fn inputs(arguments: &[String]) -> Vec<Input> {
    let mut directories = Vec::new();
    let mut libraries = Vec::new();
    let mut found = Vec::new();
    let mut at = 0;
    while at < arguments.len() {
        let argument = &arguments[at];
        let next = arguments.get(at + 1);
        if argument == "-L" {
            directories.extend(next.cloned());
            at += 2;
        } else if let Some(directory) = argument.strip_prefix("-L") {
            directories.push(directory.to_owned());
            at += 1;
        } else if argument == "-l" {
            if let Some(library) = next {
                libraries.push((at, 2, library.clone()));
            }
            at += 2;
        } else if let Some(library) = argument.strip_prefix("-l") {
            libraries.push((at, 1, library.to_owned()));
            at += 1;
        } else if argument == "-o" || argument == "-z" || argument == "-m" {
            at += 2;
        } else {
            if !argument.starts_with('-') && has_extension(argument) {
                found.push(Input {
                    at,
                    tokens: 1,
                    path: PathBuf::from(argument),
                });
            }
            at += 1;
        }
    }
    for (at, tokens, library) in libraries {
        let path = directories
            .iter()
            .map(|directory| Path::new(directory).join(format!("lib{library}.a")))
            .find(|path| path.is_file());
        if let Some(path) = path {
            found.push(Input { at, tokens, path });
        }
    }
    found.sort_by_key(|input| input.at);
    found
}

fn objects_of(
    bytes: &[u8],
    mut each: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if archive::is_archive(bytes) {
        let members = archive::members(bytes)?;
        for member in archive::objects(&members) {
            each(member.data)?;
        }
    } else if tls::is_object(bytes) {
        each(bytes)?;
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{} could not be read: {error}", path.display()))
}

pub fn through_got(
    arguments: Vec<String>,
    out: &Path,
) -> Result<(Vec<String>, Vec<PathBuf>), String> {
    let inputs = inputs(&arguments);
    let mut defined = HashSet::new();
    let mut reached = HashSet::new();
    let mut reaching = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let bytes = read(&input.path)?;
        let mut reaches = false;
        objects_of(&bytes, |object| {
            let found =
                tls::scan(object).map_err(|error| format!("{}: {error}", input.path.display()))?;
            reaches |= !found.reached.is_empty();
            defined.extend(found.defined);
            reached.extend(found.reached);
            Ok(())
        })?;
        if reaches {
            reaching.push(index);
        }
    }
    let foreign: HashSet<String> = reached.difference(&defined).cloned().collect();
    if foreign.is_empty() {
        return Ok((arguments, Vec::new()));
    }
    let mut replaced: Vec<Option<String>> = arguments.iter().cloned().map(Some).collect();
    let mut written = Vec::new();
    for index in reaching {
        let input = &inputs[index];
        let bytes = read(&input.path)?;
        let rewrite = |object: &[u8]| {
            tls::through_got(object, &foreign)
                .map_err(|error| format!("{}: {error}", input.path.display()))
        };
        let rewritten = match archive::is_archive(&bytes) {
            true => archive::rewritten(&bytes, rewrite)
                .map_err(|error| format!("{}: {error}", input.path.display()))?,
            false => rewrite(&bytes)?,
        };
        let Some(rewritten) = rewritten else {
            continue;
        };
        let extension = input
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("o");
        let mut name = out.as_os_str().to_owned();
        name.push(format!(".tls{index}.{extension}"));
        let path = PathBuf::from(name);
        std::fs::write(&path, rewritten)
            .map_err(|error| format!("{} could not be written: {error}", path.display()))?;
        replaced[input.at] = Some(path.display().to_string());
        for token in 1..input.tokens {
            replaced[input.at + token] = None;
        }
        written.push(path);
    }
    Ok((replaced.into_iter().flatten().collect(), written))
}
