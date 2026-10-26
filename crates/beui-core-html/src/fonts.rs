pub const PROPORTIONAL: &str = "Inter, system-ui, sans-serif";
pub const MONOSPACE: &str = "Hack, ui-monospace, monospace";
pub const ICONS: &str = "lucide";

pub const FACES: [(&str, &str); 3] = [
    ("Inter", "Inter-Regular.ttf"),
    ("Hack", "Hack-Regular.ttf"),
    ("lucide", "lucide.ttf"),
];

pub fn font_faces(base: &str) -> String {
    let base = base.trim_end_matches('/');
    FACES
        .iter()
        .map(|(family, file)| {
            format!(
                "@font-face {{ font-family: \"{family}\"; src: url(\"{base}/{file}\") format(\"truetype\"); font-display: block; }}\n"
            )
        })
        .collect()
}
