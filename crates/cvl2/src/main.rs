use std::{env, fs, path::Path, process::ExitCode};

use cvl2::{
    ComptimeValueBuildArtifact, Source, import_file, pretty_print_errors, printers,
    render_brackets, render_formatted, render_syntax_tree, tokenize,
};

fn write_artifact(path: &Path, artifact: &ComptimeValueBuildArtifact) -> std::io::Result<()> {
    match artifact {
        ComptimeValueBuildArtifact::File(file) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, &file.value)
        }
        ComptimeValueBuildArtifact::Folder(folder) => {
            fs::create_dir_all(path)?;
            for (name, child) in &folder.value {
                write_artifact(&path.join(name), child)?;
            }
            Ok(())
        }
    }
}

const USAGE: &str = "Usage: cvl2 [flags] <file.qxc> [out_dir]

Builds <file.qxc> and writes the result to out_dir. Without out_dir, prints the result.

Flags:
  --syntax-tree  print the parsed syntax tree
  --format       print the file reformatted
  --brackets     print the file with every node's extent marked
  --result       print the result even when writing it to out_dir";

fn main() -> ExitCode {
    let mut syntax_tree = false;
    let mut format = false;
    let mut brackets = false;
    let mut result = false;
    let mut paths = Vec::new();
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--syntax-tree" => syntax_tree = true,
            "--format" => format = true,
            "--brackets" => brackets = true,
            "--result" => result = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with("--") => {
                eprintln!("unknown flag {flag}\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
            _ => paths.push(arg),
        }
    }
    let (filename, out_dir) = match paths.as_slice() {
        [filename] => (filename, None),
        [filename, out_dir] => (filename, Some(Path::new(out_dir))),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let contents = match fs::read_to_string(filename) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {filename}: {err}");
            return ExitCode::FAILURE;
        }
    };

    if syntax_tree || format || brackets {
        let mut source = Source::new(filename.as_str(), contents.as_str());
        let tokenized = tokenize(&mut source);
        if syntax_tree {
            println!("{}", render_syntax_tree(&tokenized));
        }
        if format {
            println!("{}", render_formatted(&tokenized));
        }
        if brackets {
            println!("{}", render_brackets(&tokenized));
        }
    }

    match import_file(filename, &contents) {
        Ok(artifact) => {
            if result || out_dir.is_none() {
                println!(
                    "{}",
                    printers::printers::FOLDER_OR_FILE
                        .dump(&artifact, printers::UNLIMITED_DEPTH)
                        .trim_start()
                );
            }
            if let Some(out_dir) = out_dir
                && let Err(err) = write_artifact(out_dir, &artifact)
            {
                eprintln!("failed to write {}: {err}", out_dir.display());
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            let source = Source::new(filename.as_str(), contents.as_str());
            let preludes = cvl2::user_type::prelude_sources();
            let mut sources = vec![&source];
            sources.extend(preludes.iter());
            eprintln!("{}", pretty_print_errors(&sources, &errors).trim());
            ExitCode::FAILURE
        }
    }
}
