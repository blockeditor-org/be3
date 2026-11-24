use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use super::*;

mod products_are_named_as_they_are_parsed;
mod the_wire_round_trips;

fn round_trips<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: T,
    wire: serde_json::Value,
) {
    assert_eq!(serde_json::to_value(&value).unwrap(), wire);
    assert_eq!(serde_json::from_value::<T>(wire).unwrap(), value);
}
