use super::*;

#[test]
fn a_missing_symbol_names_it_and_its_importer() {
    let mut modules = diamond();
    modules[2]
        .imports
        .push((ENV.to_owned(), "nowhere".to_owned()));
    let error = plan(&modules, 65536, 10).unwrap_err();
    assert!(
        error.contains("plugin") && error.contains("env.nowhere"),
        "{error}"
    );
}
