pub struct Face {
    pub family: &'static str,
    pub file: &'static str,
}

pub const INTER: Face = Face {
    family: "Inter",
    file: "Inter-Regular.ttf",
};

pub const HACK: Face = Face {
    family: "Hack",
    file: "Hack-Regular.ttf",
};

pub const NOTO_EMOJI: Face = Face {
    family: "NotoEmoji",
    file: "NotoEmoji-Regular.ttf",
};

pub const LUCIDE: Face = Face {
    family: "lucide",
    file: "lucide.ttf",
};

pub const FACES: [Face; 4] = [INTER, HACK, NOTO_EMOJI, LUCIDE];

pub const INTER_REGULAR: &[u8] = include_bytes!("assets/fonts/Inter-Regular.ttf");
pub const HACK_REGULAR: &[u8] = include_bytes!("assets/fonts/Hack-Regular.ttf");
pub const NOTO_EMOJI_REGULAR: &[u8] = include_bytes!("assets/fonts/NotoEmoji-Regular.ttf");
pub const ICONS: &[u8] = include_bytes!("assets/icons/lucide.ttf");

pub fn bytes(file: &str) -> Option<&'static [u8]> {
    Some(match file {
        "Inter-Regular.ttf" => INTER_REGULAR,
        "Hack-Regular.ttf" => HACK_REGULAR,
        "NotoEmoji-Regular.ttf" => NOTO_EMOJI_REGULAR,
        "lucide.ttf" => ICONS,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
