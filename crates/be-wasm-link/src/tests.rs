use super::*;

fn arguments(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn library(name: &str, needs: &[&str], linked: &[&str], functions: &[&str]) -> Library {
    Library {
        name: name.to_owned(),
        identity: name.repeat(64 / name.len()),
        needs: needs
            .iter()
            .map(|need| need.repeat(64 / need.len()))
            .collect(),
        linked: linked.iter().map(|archive| (*archive).to_owned()).collect(),
        functions: functions
            .iter()
            .map(|function| (*function).to_owned())
            .collect(),
        globals: HashSet::from([format!("{}_DATA", name.to_uppercase())]),
    }
}

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

mod a_crate_in_two_modules_of_a_closure_is_refused;
mod a_library_the_link_does_not_name_is_refused;
mod a_linked_modules_mangled_symbols_are_shortened;
mod a_missing_symbol_names_it_and_its_importer;
mod a_response_file_is_split_as_lld_splits_it;
mod a_root_link_takes_every_rust_object_whole;
mod a_root_link_takes_libcxx_whole_once;
mod a_symbol_two_libraries_define_is_refused;
mod an_app_link_leaves_out_what_its_libraries_link;
mod exported_globals_are_added_to_a_module;
mod libraries_are_ordered_below_what_needs_them;
mod the_library_listing_is_read_back;

fn importing(imports: &[(&str, &str, u8)]) -> Vec<u8> {
    let mut entries = leb(imports.len() as u64);
    for (module, name, kind) in imports {
        entries.extend(named(module));
        entries.extend(named(name));
        entries.push(*kind);
        match *kind {
            module::FUNCTION => entries.push(0),
            _ => entries.extend([0x7f, 1]),
        }
    }
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.extend(section(1, &[1, 0x60, 0, 0]));
    bytes.extend(section(2, &entries));
    bytes
}
