use super::*;

fn format(code: Fourcc, modifier: Modifier) -> Format {
    Format { code, modifier }
}

#[test]
fn a_display_only_device_gets_buffers_in_a_format_both_sides_share() {
    let tiled = Modifier::from(0x0800_0000_0000_0001);
    let plane = [
        format(Fourcc::Xrgb8888, Modifier::Linear),
        format(Fourcc::Rgb565, Modifier::Linear),
        format(Fourcc::R8, Modifier::Linear),
    ];
    let renderer = [
        format(Fourcc::Argb8888, Modifier::Linear),
        format(Fourcc::Xrgb8888, tiled),
        format(Fourcc::Xrgb8888, Modifier::Linear),
        format(Fourcc::Xrgb8888, Modifier::Invalid),
    ];
    assert_eq!(
        chosen_formats(&plane, &renderer),
        Some((Fourcc::Xrgb8888, vec![Modifier::Linear])),
        "only the layouts the display can read are allocated, and never an implicit one"
    );
    assert_eq!(
        chosen_formats(&plane[1..], &renderer),
        None,
        "a display that reads none of the GPU's formats gets none"
    );
}
