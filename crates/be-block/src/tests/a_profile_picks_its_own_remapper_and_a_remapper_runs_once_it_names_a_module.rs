use super::*;

use crate::input_settings::RemapperChoice;
use crate::settings::Layered;
use crate::{InputRemapper, InputRemapperContent};

#[test]
fn a_profile_picks_its_own_remapper_and_a_remapper_runs_once_it_names_a_module() {
    let editor = Uuid::new_v4();
    let shared_choice = RemapperChoice {
        block: Uuid::new_v4(),
        editor,
    };
    let own_choice = RemapperChoice {
        block: Uuid::new_v4(),
        editor,
    };
    let shared = edited(
        &InputSettingsContent::default(),
        [InputSettings::set_remapper(Some(shared_choice))],
    )
    .root();
    let own = edited(
        &InputSettingsContent::default(),
        [InputSettings::set_remapper(Some(own_choice))],
    )
    .root();
    assert_eq!(
        InputSettings::default().over(&shared).remapper,
        Some(shared_choice)
    );
    assert_eq!(own.over(&shared).remapper, Some(own_choice));

    let remapper = InputRemapperContent::default();
    assert_eq!(remapper.root().runs(), None, "a new remapper runs nothing");
    let remapper = edited(
        &remapper,
        [InputRemapper::set(
            "be3.input-remaps",
            "remapper.wasm",
            Some(b"[[group]]".to_vec()),
        )],
    );
    assert_eq!(
        remapper.root().runs(),
        Some(("be3.input-remaps", "remapper.wasm", &b"[[group]]"[..]))
    );
    let cleared = edited(
        &remapper,
        [InputRemapper::set(
            "be3.input-remaps",
            "remapper.wasm",
            None,
        )],
    );
    assert_eq!(
        cleared.root().runs(),
        None,
        "a remapper with no config runs nothing"
    );
}
