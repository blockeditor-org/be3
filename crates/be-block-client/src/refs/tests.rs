use serde::{Serialize, de::DeserializeOwned};

use super::*;

mod every_record_reads_back_with_each_option_none_and_some;

/// Writes `value` as a refs row holds it and reads it back, failing unless it reads back to the
/// same bytes.
#[track_caller]
fn reads_back<T: Serialize + DeserializeOwned>(value: &T) {
    let written = encode(value);
    let read: T = decode(&written).expect("the record reads back");
    assert_eq!(
        encode(&read),
        written,
        "the record reads back as it was written"
    );
}
