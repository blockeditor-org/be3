use super::*;

#[test]
fn tiles_past_the_source_share_one_download() {
    let mut worker = read_worker();
    let deep = |x, y| TileId { zoom: 16, x, y };
    let source = TileId {
        zoom: MAX_ZOOM,
        x: 10,
        y: 20,
    };

    worker.request(deep(40, 80));
    worker.request(deep(43, 83));

    assert_eq!(deep(43, 83).source(MAX_ZOOM), source);
    assert_eq!(worker.queued, vec![source]);
    assert_eq!(worker.waiting[&source], vec![deep(40, 80), deep(43, 83)]);
}
