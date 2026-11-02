use super::*;

#[test]
fn monitors_sort_primary_first_and_are_named() {
    let mut list = vec![
        monitor(1, "\\\\.\\DISPLAY3", Rect::new(-1920, 0, 0, 1080), false),
        monitor(2, "\\\\.\\DISPLAY2", Rect::new(1920, 0, 3840, 1080), false),
        monitor(3, "\\\\.\\DISPLAY1", Rect::new(0, 0, 1920, 1080), true),
    ];
    monitors::sort(&mut list);
    let order: Vec<u64> = list.iter().map(|monitor| monitor.handle.0).collect();
    assert_eq!(order, [3, 1, 2]);
    assert_eq!(
        monitors::name(Some("DELL U2720Q"), Some("Generic PnP Monitor"), 0),
        "DELL U2720Q"
    );
    assert_eq!(
        monitors::name(Some(" "), Some("Generic PnP Monitor"), 0),
        "Generic PnP Monitor"
    );
    assert_eq!(monitors::name(None, None, 1), "Display 2");
}
