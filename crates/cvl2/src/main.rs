use std::{env, fs, path::Path, process::ExitCode};

use cvl2::{
    ComptimeValueBuildArtifact, Source, import_file, pretty_print_errors, printers,
    render_tokenized_output, tokenize,
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

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (filename, out_dir) = match args.as_slice() {
        [filename] => (filename, None),
        [filename, out_dir] => (filename, Some(Path::new(out_dir))),
        _ => {
            eprintln!("Usage: cvl2 <file.qxc> [out_dir]");
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

    let mut source = Source::new(filename.as_str(), contents.as_str());
    let tokenized = tokenize(&mut source);
    println!("{}", render_tokenized_output(&tokenized, &source));

    match import_file(filename, &contents) {
        Ok(artifact) => {
            println!(
                "got result{}",
                printers::printers::FOLDER_OR_FILE.dump(&artifact, printers::UNLIMITED_DEPTH)
            );
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
            let unreported: Vec<_> = errors
                .into_iter()
                .filter(|err| !tokenized.errors.contains(err))
                .collect();
            println!("{}", pretty_print_errors(&sources, &unreported));
            ExitCode::FAILURE
        }
    }
}
