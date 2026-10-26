use crate::Image;

/// An icon's colour bitmap as `GetDIBits` gives it, 32 bits a pixel in BGRA,
/// top row first, into rgba. An icon without alpha takes it from its AND
/// mask, read the same way: a pixel set in the mask is transparent.
pub fn from_bgra(width: u32, height: u32, bgra: &[u8], mask: Option<&[u8]>) -> Option<Image> {
    let pixels = usize::try_from(width).ok()? * usize::try_from(height).ok()?;
    if pixels == 0 || bgra.len() < pixels * 4 {
        return None;
    }
    let bgra = &bgra[..pixels * 4];
    let has_alpha = bgra.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mask = mask.filter(|mask| mask.len() >= pixels * 4);
    let rgba = bgra
        .chunks_exact(4)
        .enumerate()
        .flat_map(|(index, pixel)| {
            let alpha = match (has_alpha, mask) {
                (true, _) => pixel[3],
                (false, Some(mask)) if mask[index * 4..index * 4 + 3] != [0, 0, 0] => 0,
                (false, _) => 255,
            };
            [pixel[2], pixel[1], pixel[0], alpha]
        })
        .collect();
    Some(Image {
        width,
        height,
        rgba,
    })
}

/// Undoes premultiplied alpha, as the shell's image factory gives it.
pub fn unpremultiply(image: &mut Image) {
    for pixel in image.rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            *channel = (u32::from(*channel) * 255 / alpha).min(255) as u8;
        }
    }
}
