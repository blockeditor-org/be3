use super::*;

mod a_module_fetched_after_the_remapper_changed_is_not_installed;
mod a_module_is_fetched_once_and_a_new_config_only_reconfigures_it;

fn wanted(module: &str, config: &[u8]) -> Wanted {
    Wanted {
        plugin: "be3.settings".to_owned(),
        module: module.to_owned(),
        config: config.to_vec(),
    }
}
