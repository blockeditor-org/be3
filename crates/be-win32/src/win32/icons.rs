use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, HGDIOBJ, ReleaseDC,
};
use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, HICON, ICONINFO};

use crate::{Image, icon};

// unsafe(ffi)
pub fn from_hicon(hicon: HICON) -> Option<Image> {
    let mut info = ICONINFO::default();
    unsafe { GetIconInfo(hicon, &mut info) }.ok()?;
    let image = (!info.hbmColor.0.is_null())
        .then(|| {
            let (width, height) = size(info.hbmColor)?;
            let color = bits(info.hbmColor, width, height)?;
            let mask = bits(info.hbmMask, width, height);
            icon::from_bgra(width, height, &color, mask.as_deref())
        })
        .flatten();
    for bitmap in [info.hbmColor, info.hbmMask] {
        if !bitmap.0.is_null() {
            let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        }
    }
    image
}

// unsafe(ffi)
pub fn from_premultiplied(bitmap: HBITMAP) -> Option<Image> {
    let image = size(bitmap).and_then(|(width, height)| {
        let color = bits(bitmap, width, height)?;
        let mut image = icon::from_bgra(width, height, &color, None)?;
        icon::unpremultiply(&mut image);
        Some(image)
    });
    let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
    image
}

// unsafe(ffi)
fn size(bitmap: HBITMAP) -> Option<(u32, u32)> {
    let mut info = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some((&mut info as *mut BITMAP).cast()),
        )
    };
    if read == 0 {
        return None;
    }
    Some((
        u32::try_from(info.bmWidth).ok()?,
        u32::try_from(info.bmHeight).ok()?,
    ))
}

// unsafe(ffi)
fn bits(bitmap: HBITMAP, width: u32, height: u32) -> Option<Vec<u8>> {
    if bitmap.0.is_null() {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: i32::try_from(width).ok()?,
            biHeight: -i32::try_from(height).ok()?,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; usize::try_from(width).ok()? * usize::try_from(height).ok()? * 4];
    let dc = unsafe { GetDC(None) };
    let lines = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, dc) };
    (u32::try_from(lines).ok() == Some(height)).then_some(pixels)
}
