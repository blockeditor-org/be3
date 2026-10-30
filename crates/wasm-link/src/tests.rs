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

fn leb(value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    module::unsigned(value, &mut out);
    out
}

fn named(text: &str) -> Vec<u8> {
    let mut out = leb(text.len() as u64);
    out.extend_from_slice(text.as_bytes());
    out
}

fn section(id: u8, contents: &[u8]) -> Vec<u8> {
    let mut out = vec![id];
    out.extend(leb(contents.len() as u64));
    out.extend_from_slice(contents);
    out
}

fn custom(title: &str, payload: &[u8]) -> Vec<u8> {
    let mut contents = named(title);
    contents.extend_from_slice(payload);
    section(0, &contents)
}

const TLS_SLEB: u8 = 21;

fn reaching_object(symbol: &str, addend: u8) -> Vec<u8> {
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    out.extend(section(1, &[1, 0x60, 0, 1, 0x7f]));
    let mut imports = vec![1];
    imports.extend(named("env"));
    imports.extend(named("__tls_base"));
    imports.extend([3, 0x7f, 1]);
    out.extend(section(2, &imports));
    out.extend(section(3, &[1, 0]));
    let body = [
        0, 0x23, 0x80, 0x80, 0x80, 0x80, 0, 0x41, 0x80, 0x80, 0x80, 0x80, 0, 0x6a, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    out.extend(section(10, &code));
    let mut table = vec![3, 0, 0, 0];
    table.extend(named("reach"));
    table.extend([2, 0x10, 0]);
    table.extend([1, 0x10]);
    table.extend(named(symbol));
    let mut linking = vec![2, 8];
    linking.extend(leb(table.len() as u64));
    linking.extend(table);
    out.extend(custom("linking", &linking));
    out.extend(custom(
        "reloc.CODE",
        &[3, 2, 7, 4, 1, TLS_SLEB, 10, 2, addend],
    ));
    out
}

fn archive(members: &[(&str, &[u8])], symbol: &str) -> Vec<u8> {
    let header = |name: &str, size: usize| {
        format!("{name:<16}{:<12}{:<6}{:<6}{:<8}{size:<10}`\n", 0, 0, 0, 644).into_bytes()
    };
    let mut table = vec![0, 0, 0, 1, 0, 0, 0, 0];
    table.extend_from_slice(symbol.as_bytes());
    table.push(0);
    let mut out = b"!<arch>\n".to_vec();
    out.extend(header("/", table.len()));
    let first = out.len() + table.len() + table.len() % 2;
    table[4..8].copy_from_slice(&(first as u32).to_be_bytes());
    out.extend(&table);
    if table.len() % 2 == 1 {
        out.push(b'\n');
    }
    for (name, data) in members {
        out.extend(header(name, data.len()));
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    out
}

mod a_plugin_link_has_the_sdk_thread_locals_first;
mod a_plugin_link_leaves_out_what_the_sdk_links;
mod a_plugin_thread_local_among_the_sdks_is_refused;
mod a_response_file_is_split_as_lld_splits_it;
mod a_thread_local_another_module_defines_is_read_through_got_tls;
mod a_thread_local_defined_in_the_link_stays_local;
mod an_archive_is_rewritten_with_its_symbol_table_moved;
mod an_import_the_sdk_does_not_export_is_reported;
mod an_offset_into_another_modules_thread_local_is_refused;
mod an_sdk_link_takes_every_rust_object_whole;
mod an_sdk_link_takes_libcxx_whole_once;
mod exported_globals_are_added_to_a_module;
mod the_link_map_gives_the_thread_locals_in_order;
mod the_plugins_reserve_must_be_the_sdks_last_thread_local;
mod the_sdk_listing_is_read_back;
