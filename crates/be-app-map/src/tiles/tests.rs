use super::*;

mod a_tile_json_without_a_max_zoom_serves_every_zoom;
mod cancelling_the_last_waiting_tile_unqueues_its_source;
mod queue_serves_latest_requests_first;
mod tiles_asked_for_before_the_source_is_read_wait_for_it;
mod tiles_past_the_source_share_one_download;

const MAX_ZOOM: u8 = 14;

/// A worker that has read its tile source.
fn read_worker() -> TileWorker {
    let mut worker = TileWorker::spawn(Waker::default());
    worker.read(TileSource {
        template: "https://tiles.openfreemap.org/planet/test/{z}/{x}/{y}.pbf".to_owned(),
        max_zoom: MAX_ZOOM,
    });
    worker
}
