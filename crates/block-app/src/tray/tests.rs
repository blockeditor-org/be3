use std::collections::HashMap;

use block_plugin_api::{TrayMenuItem, TrayStatus, TrayToggle};
use zbus::zvariant::{OwnedValue, Value};

use super::model::*;

mod a_menu_keeps_what_is_shown_without_its_mnemonics;
mod a_service_names_the_bus_and_path_of_its_item;
mod an_item_reads_its_properties;
mod items_are_published_once_read_and_go_with_their_owner;
mod items_on_a_private_bus_show_and_answer_the_desktop;
mod the_pixmap_nearest_above_the_size_is_turned_to_rgba;

fn owned(value: Value<'_>) -> OwnedValue {
    value
        .try_into_owned()
        .expect("a test value has no file descriptors")
}

fn entry(
    id: i32,
    properties: Vec<(&'static str, Value<'static>)>,
    children: Vec<Value<'static>>,
) -> Value<'static> {
    let properties: HashMap<&str, Value<'static>> = properties.into_iter().collect();
    Value::from((id, properties, children))
}
