use super::*;

fn arguments(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn sdk(linked: &[&str]) -> Sdk {
    Sdk {
        identity: "cafe".to_owned(),
        linked: linked.iter().map(|name| (*name).to_owned()).collect(),
        importable: HashSet::from(["sdk_function".to_owned()]),
        exports: HashSet::from(["sdk_function".to_owned(), "SDK_DATA".to_owned()]),
        thread_locals: vec![ThreadLocal {
            name: "SDK_LOCAL".to_owned(),
            offset: 8,
            size: 4,
        }],
        reserve: (16, 32),
    }
}

const MAP: &str = "    Addr      Off     Size Out     In      Symbol
       -        8      365 TYPE
  100000    a209e       e0 .tdata
  100000    a20a1        4         libsdk.rlib(sdk.o):(.tdata.LOCAL)
       0    a20a1        4                 LOCAL
  100004    a20a5        8         libc.a(errno.o):(.tbss.errno)
       4    a20a5        8                 errno
  1000e0    a2181    11180 .rodata
  1000e0    a2185       16         libsdk.rlib(sdk.o):(.rodata.x)
  1000e0    a2185       16                 .Lanon.1
";

mod a_plugin_link_has_the_sdk_thread_locals_first;
mod a_plugin_link_leaves_out_what_the_sdk_links;
mod a_plugin_thread_local_among_the_sdks_is_refused;
mod a_response_file_is_split_as_lld_splits_it;
mod an_import_the_sdk_does_not_export_is_reported;
mod an_sdk_link_takes_every_rust_object_whole;
mod an_sdk_link_takes_libcxx_whole_once;
mod exported_globals_are_added_to_a_module;
mod the_link_map_gives_the_thread_locals_in_order;
mod the_plugins_reserve_must_be_the_sdks_last_thread_local;
mod the_sdk_listing_is_read_back;
