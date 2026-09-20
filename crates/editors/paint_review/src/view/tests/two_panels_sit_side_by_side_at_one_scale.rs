use super::*;

use block_editor_plugin::beui::{Image, pos2};

#[test]
fn two_panels_sit_side_by_side_at_one_scale() {
    let panels = [rendered(20, 10), rendered(20, 10)];
    let view = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(104.0, 40.0));

    let layout = laid_out(&panels, view);

    assert_eq!(layout.scale, 2.0);
    assert_eq!(layout.panels[0].rect.min, pos2(0.0, 10.0));
    assert_eq!(layout.panels[1].rect.min, pos2(64.0, 10.0));
    assert_eq!(layout.panels[1].rect.max, pos2(104.0, 30.0));
}

fn rendered(width: u32, height: u32) -> Rendered {
    let image = Image::from_rgba(width, height, vec![0; width as usize * height as usize * 4]);
    Rendered {
        size: image.size(),
        image,
        description: String::new(),
    }
}
