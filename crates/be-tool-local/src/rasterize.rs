use std::path::{Path, PathBuf};

use beui_paint_snapshot::Painting;

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
    let painting = Painting::decode(&bytes)
        .unwrap_or_else(|error| fail(&format!("{input} is unreadable: {error}")));

    let kept = painting.frames();
    let frames = match wanted {
        Wanted::All => 0..kept,
        Wanted::Frame(index) if index < kept => index..index + 1,
        Wanted::Frame(index) => fail(&format!(
            "{input} holds {}, numbered from 0, so there is no frame {index}",
            match kept {
                1 => "one frame".to_owned(),
                kept => format!("{kept} frames"),
            }
        )),
    };
    let numbered = matches!(wanted, Wanted::All) && kept > 1;
    for index in frames {
        let image = painting
            .render(
                index,
                Some(Path::new(input)),
                Some(&mut crate::browser::renderer(None)),
                beui_paint_raster::render,
            )
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
    eprintln!("usage: be-tool-local rasterize SNAPSHOT.paint OUTPUT.png [FRAME|all]");
    std::process::exit(2)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
