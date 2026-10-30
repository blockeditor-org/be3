use super::*;

fn sdk(reserve: i32) -> Sdk {
    Sdk {
        functions: HashSet::from(["sdk_function".to_owned()]),
        addresses: HashMap::from([
            ("SDK_DATA".to_owned(), 4096),
            (PLUGIN_TLS.to_owned(), reserve),
        ]),
    }
}

const IMPORTS: [(&str, &str); 7] = [
    (ENV, TABLE),
    (ENV, MEMORY_BASE),
    (ENV, "sdk_function"),
    (GOT_FUNCTION, "sdk_function"),
    (GOT_FUNCTION, "own_function"),
    (GOT_MEMORY, "SDK_DATA"),
    (GOT_MEMORY, "OWN_DATA"),
];

mod a_plan_places_the_plugin_after_the_sdk;
mod a_thread_local_slot_is_relative_to_the_importing_block;
mod the_declared_layout_is_read_from_an_export;
mod thread_locals_past_the_reserve_are_copied;
