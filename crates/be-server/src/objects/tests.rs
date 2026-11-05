use std::time::Duration;

use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::*;
use crate::TestBucket;

mod a_bucket_store_round_trips_objects;
mod a_presigned_put_takes_only_the_object_it_was_signed_for;
mod a_tampered_object_reads_as_missing;
mod the_cache_in_front_of_a_bucket_keeps_only_its_limit;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build()
}
