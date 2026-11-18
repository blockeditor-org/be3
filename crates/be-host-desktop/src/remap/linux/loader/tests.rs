use super::*;

mod a_config_changed_while_the_module_compiles_waits_for_it;
mod a_kept_module_that_will_not_compile_is_fetched;
mod a_module_changed_while_another_compiles_is_fetched_and_the_other_dropped;
mod a_module_fetched_after_the_remapper_changed_is_not_installed;
mod a_module_is_fetched_once_and_a_new_config_only_reconfigures_it;
mod a_module_that_failed_is_fetched_again_with_the_next_change;

fn wanted(module: &str, config: &[u8]) -> Wanted {
    Wanted {
        app: "be3.settings".to_owned(),
        module: module.to_owned(),
        config: config.to_vec(),
    }
}
