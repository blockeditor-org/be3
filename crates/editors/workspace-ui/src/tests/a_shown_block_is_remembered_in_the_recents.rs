use block_editor_beui::be_block::Recents;
use block_editor_beui::be_block::profile::RECENTS;

use super::*;

#[test]
fn a_shown_block_is_remembered_in_the_recents() {
    let (mut fixture, opened) = profiled(None);
    let second = Uuid::new_v4();

    show(&mut fixture, opened, None);
    show(&mut fixture, second, Some(opened));

    let recents: Vec<Uuid> = profile(&fixture)
        .state(RECENTS)
        .and_then(ViewState::value::<Recents>)
        .expect("the profile keeps recents")
        .0
        .into_iter()
        .map(|recent| recent.block)
        .collect();
    assert_eq!(
        recents,
        vec![second, opened],
        "the block shown last comes first"
    );
}
