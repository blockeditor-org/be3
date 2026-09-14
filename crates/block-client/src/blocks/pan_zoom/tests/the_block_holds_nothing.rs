use super::*;

#[test]
fn the_block_holds_nothing() {
    assert_eq!(std::mem::size_of::<PanZoom>(), 0);
    assert_eq!(serde_json::to_string(&PanZoom::default()).unwrap(), "{}");
}
