use std::path::{Path, PathBuf};

use paint_snapshot::Snapshot;

pub fn main(arguments: Vec<String>) -> std::process::ExitCode {
    let (input, output, wanted) = match arguments.as_slice() {
        [input, output] => (input, output, Wanted::Frame(0)),
        [input, output, frame] if frame == "all" => (input, output, Wanted::All),
        [input, output, frame] => match frame.parse() {
            Ok(index) => (input, output, Wanted::Frame(index)),
            Err(_) => usage(),
        },
        _ => usage(),
    };

    let bytes = std::fs::read(input)
        .unwrap_or_else(|error| fail(&format!("could not read {input}: {error}")));
    if paint_snapshot::BrowserSnapshot::is_browser(&bytes) {
        if !matches!(wanted, Wanted::Frame(0) | Wanted::All) {
            fail("a browser painting has only frame 0");
        }
        crate::browser::rasterize(Path::new(input), Path::new(output), None)
            .unwrap_or_else(|error| fail(&error));
        return std::process::ExitCode::SUCCESS;
    }
    let snapshot = Snapshot::decode(&bytes)
        .unwrap_or_else(|error| fail(&format!("{input} is unreadable: {error}")));

    let kept = snapshot.frames.len();
    let frames = match wanted {
        Wanted::All => 0..kept,
        Wanted::Frame(index) if index < kept => index..index + 1,
        Wanted::Frame(index) => fail(&format!(
            "{input} holds {kept} frames, numbered from 0, so there is no frame {index}"
        )),
    };
    let numbered = matches!(wanted, Wanted::All) && kept > 1;
    for index in frames {
        let image = paint_snapshot::render(&snapshot, index)
            .unwrap_or_else(|error| fail(&format!("frame {index} did not render: {error}")));
        let path = match numbered {
            true => numbered_path(Path::new(output), index),
            false => PathBuf::from(output),
        };
        image
            .save(&path)
            .unwrap_or_else(|error| fail(&format!("could not write {}: {error}", path.display())));
        println!("{} is {}x{}", path.display(), image.width(), image.height());
    }
    std::process::ExitCode::SUCCESS
}

enum Wanted {
    Frame(usize),
    All,
}

fn numbered_path(output: &Path, index: usize) -> PathBuf {
    let extension = output.extension().unwrap_or_default().to_string_lossy();
    let stem = output.with_extension("");
    PathBuf::from(format!("{}.{index:02}.{extension}", stem.display()))
}

fn usage() -> ! {
    eprintln!("usage: local-tools rasterize SNAPSHOT.paint OUTPUT.png [FRAME|all]");
    std::process::exit(2)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
