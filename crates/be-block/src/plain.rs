const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

pub fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => extension.to_ascii_lowercase(),
        _ => String::new(),
    }
}

const MEDIA_TYPES: [(&str, &str); 18] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("tiff", "image/tiff"),
    ("tif", "image/tiff"),
    ("ico", "image/x-icon"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/wav"),
    ("ogg", "audio/ogg"),
    ("flac", "audio/flac"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("ttf", "font/ttf"),
    ("wasm", "application/wasm"),
    ("epub", "application/epub+zip"),
];

pub fn media_type_of(name: &str) -> &'static str {
    let extension = extension(name);
    MEDIA_TYPES
        .iter()
        .find(|(known, _)| *known == extension)
        .map_or("application/octet-stream", |(_, media_type)| media_type)
}

pub fn extension_of(media_type: &str) -> &'static str {
    match media_type {
        "audio/x-wav" => "wav",
        media_type => MEDIA_TYPES
            .iter()
            .find(|(_, known)| *known == media_type)
            .map_or("", |(extension, _)| extension),
    }
}

pub fn export_name(name: &str, extension: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|character| match character {
            character if FORBIDDEN.contains(&character) => '-',
            character => character,
        })
        .collect();
    let stem = match stem.is_empty() {
        true => "Untitled".to_owned(),
        false => stem,
    };
    match extension.is_empty() || self::extension(&stem) == extension {
        true => stem,
        false => format!("{stem}.{extension}"),
    }
}
