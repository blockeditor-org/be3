use super::*;
use be_worker_map::SOURCE_MAX_ZOOM;

mod cancelling_the_last_waiting_tile_unqueues_its_source;
mod queue_serves_latest_requests_first;
mod tiles_past_the_source_share_one_download;
