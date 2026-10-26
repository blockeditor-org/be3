use super::*;

mod a_cursor_missing_from_a_theme_is_found_in_the_themes_it_inherits;
mod the_environment_picks_the_theme_its_size_and_where_to_look;
mod the_image_nearest_the_wanted_size_is_loaded_with_its_hotspot;

pub(crate) struct Icons(pub(crate) PathBuf);

impl Icons {
    pub(crate) fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("beui-cursors-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("the scratch directory is made");
        Self(directory)
    }

    pub(crate) fn index(&self, theme: &str, contents: &str) {
        let directory = self.0.join(theme);
        std::fs::create_dir_all(&directory).expect("the theme's directory is made");
        std::fs::write(directory.join("index.theme"), contents).expect("the index is written");
    }

    pub(crate) fn cursor(&self, theme: &str, name: &str, images: &[Drawn]) {
        let directory = self.0.join(theme).join("cursors");
        std::fs::create_dir_all(&directory).expect("the cursors directory is made");
        std::fs::write(directory.join(name), xcursor_file(images)).expect("the cursor is written");
    }

    pub(crate) fn theme(&self, name: &str, size: u32) -> Theme {
        Theme::new(name, size, vec![self.0.clone()])
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Drawn {
    pub(crate) nominal: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) hotspot: (u32, u32),
    pub(crate) argb: u32,
}

pub(crate) fn drawn(nominal: u32, hotspot: (u32, u32), argb: u32) -> Drawn {
    Drawn {
        nominal,
        width: nominal,
        height: nominal,
        hotspot,
        argb,
    }
}

const IMAGE: u32 = 0xfffd_0002;

fn xcursor_file(images: &[Drawn]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let word = |bytes: &mut Vec<u8>, value: u32| bytes.extend_from_slice(&value.to_le_bytes());
    bytes.extend_from_slice(b"Xcur");
    word(&mut bytes, 16);
    word(&mut bytes, 0x0001_0000);
    word(&mut bytes, images.len() as u32);
    let mut position = 16 + 12 * images.len() as u32;
    for image in images {
        word(&mut bytes, IMAGE);
        word(&mut bytes, image.nominal);
        word(&mut bytes, position);
        position += 36 + image.width * image.height * 4;
    }
    for image in images {
        word(&mut bytes, 36);
        word(&mut bytes, IMAGE);
        word(&mut bytes, image.nominal);
        word(&mut bytes, 1);
        word(&mut bytes, image.width);
        word(&mut bytes, image.height);
        word(&mut bytes, image.hotspot.0);
        word(&mut bytes, image.hotspot.1);
        word(&mut bytes, 0);
        for _ in 0..image.width * image.height {
            word(&mut bytes, image.argb);
        }
    }
    bytes
}
