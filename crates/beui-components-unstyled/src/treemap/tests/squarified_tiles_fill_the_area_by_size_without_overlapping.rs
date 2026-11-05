use super::*;

#[test]
fn squarified_tiles_fill_the_area_by_size_without_overlapping() {
    let sizes = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
    let rects = squarify(&sizes, area);
    assert_eq!(rects.len(), sizes.len());
    let total: f64 = sizes.iter().sum();
    for (rect, size) in rects.iter().zip(sizes) {
        let share = f64::from(rect.width() * rect.height()) / f64::from(600.0_f32 * 400.0);
        assert!(
            (share - size / total).abs() < 1e-3,
            "{rect:?} is not {size} of {total}"
        );
        assert!(rect.min.x >= -1e-3 && rect.min.y >= -1e-3);
        assert!(rect.max.x <= 600.0 + 1e-3 && rect.max.y <= 400.0 + 1e-3);
        let ratio = rect.width().max(rect.height()) / rect.width().min(rect.height());
        assert!(ratio < 3.0, "{rect:?} is far from square");
    }
    for (index, a) in rects.iter().enumerate() {
        for b in &rects[index + 1..] {
            let overlap = (a.max.x.min(b.max.x) - a.min.x.max(b.min.x)).max(0.0)
                * (a.max.y.min(b.max.y) - a.min.y.max(b.min.y)).max(0.0);
            assert!(overlap < 1e-2, "{a:?} overlaps {b:?}");
        }
    }
}
