use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

mod a_block_is_written_once_to_a_stable_path_and_what_the_program_saved_is_taken_back;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "block-app-program-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the scratch directory is made");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn book(bytes: &[u8]) -> PlainFile {
    PlainFile {
        bytes: bytes.to_vec(),
        name: Some("Moby Dick.epub".to_owned()),
        extension: "epub".to_owned(),
        media_type: "application/epub+zip".to_owned(),
    }
}
