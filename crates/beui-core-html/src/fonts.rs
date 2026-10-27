use beui_fonts::{FACES, Face, NOTO_EMOJI};

pub const PROPORTIONAL: &str = "Inter, NotoEmoji, system-ui, sans-serif";
pub const MONOSPACE: &str = "Hack, NotoEmoji, ui-monospace, monospace";
pub const ICONS: &str = "lucide";

const EMOJI: &str =
    "U+200D, U+20E3, U+2100-2BFF, U+3030, U+303D, U+3297, U+3299, U+FE0F, U+1F000-1FAFF";

fn unicode_range(face: &Face) -> String {
    if face.file == NOTO_EMOJI.file {
        format!(" unicode-range: {EMOJI};")
    } else {
        String::new()
    }
}

pub fn font_faces(base: &str) -> String {
    let base = base.trim_end_matches('/');
    FACES
        .iter()
        .map(|face| {
            format!(
                "@font-face {{ font-family: {}; src: url(\"{base}/{}\") format(\"truetype\"); font-display: block;{} }}\n",
                face.family,
                face.file,
                unicode_range(face)
            )
        })
        .collect()
}
