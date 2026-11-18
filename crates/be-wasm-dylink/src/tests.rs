use super::*;

fn module(
    name: &str,
    layout: &str,
    imports: &[(&str, &str)],
    functions: &[&str],
    globals: &[&str],
) -> Described {
    Described {
        name: name.to_owned(),
        imports: imports
            .iter()
            .map(|(module, name)| ((*module).to_owned(), (*name).to_owned()))
            .collect(),
        functions: functions.iter().map(|name| (*name).to_owned()).collect(),
        globals: globals.iter().map(|name| (*name).to_owned()).collect(),
        exports: [layout]
            .into_iter()
            .chain(functions.iter().copied())
            .chain(globals.iter().copied())
            .map(str::to_owned)
            .collect(),
    }
}

fn diamond() -> Vec<Described> {
    vec![
        module("root", "", &[], &["malloc"], &["ROOT_DATA"]),
        module(
            "left",
            "__be3_dylink_100_4_3_2",
            &[(ENV, TABLE), (ENV, MEMORY_BASE), (ENV, "malloc")],
            &["left_function"],
            &["LEFT_DATA"],
        ),
        module(
            "app",
            "__be3_dylink_8_0_1_0",
            &[
                (ENV, "left_function"),
                (GOT_FUNCTION, "malloc"),
                (GOT_FUNCTION, "own_function"),
                (GOT_MEMORY, "ROOT_DATA"),
                (GOT_MEMORY, "LEFT_DATA"),
                (GOT_MEMORY, "OWN_DATA"),
                ("be3_host", "host_wake"),
            ],
            &["own_function"],
            &["OWN_DATA"],
        ),
    ]
}

mod a_library_that_is_not_what_it_is_named_is_refused;
mod a_mangled_symbol_is_shortened_and_others_are_kept;
mod a_missing_symbol_names_it_and_its_importer;
mod a_plan_places_every_module_after_the_ones_it_needs;
mod the_declared_layout_is_read_from_an_export;
mod the_libraries_are_read_in_dependency_order;
