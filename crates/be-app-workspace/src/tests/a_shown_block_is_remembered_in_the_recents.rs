use be_sdk_beui::be_block::Recents;

use super::*;

#[test]
fn a_shown_block_is_remembered_in_the_recents() {
    let (mut fixture, opened) = profiled(None);
    let second = Uuid::new_v4();

    show(&mut fixture, opened, None);
    show(&mut fixture, second, Some(opened));

    let recents: Vec<Uuid> = profile(&fixture)
        .view_state::<Recents>()
        .and_then(Result::ok)
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
