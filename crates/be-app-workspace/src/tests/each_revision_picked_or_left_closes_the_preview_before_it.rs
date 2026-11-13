use super::*;
use be_sdk_beui::Revision;
use be_sdk_test::HostCall;

fn revision(commit: u8, current: bool) -> Revision {
    Revision {
        commit: [commit; 32],
        time: 1_800_000_000_000 + i64::from(commit) * 1_000,
        current,
    }
}

fn preview(fixture: &mut Fixture, commit: u8) {
    fixture.test.click(format!(
        "workspace.revision.{}",
        format!("{commit:02x}").repeat(32)
    ));
    fixture.settle();
    let asked = fixture
        .test
        .take_revision_previews()
        .pop()
        .expect("picking a revision asks for its preview");
    assert_eq!(asked.commit, [commit; 32]);
    fixture
        .test
        .answer(asked.reply, Ok(Uuid::new_v4().into_bytes()));
    fixture.settle();
}

#[test]
fn each_revision_picked_or_left_closes_the_preview_before_it() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1100.0, 720.0)));
    show(&mut fixture, opened, None);
    let listing = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ListRevisions { reply, block } if block == opened.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("the active file lists its history");
    fixture.test.answer(
        listing,
        Ok(vec![
            revision(1, false),
            revision(2, false),
            revision(3, true),
        ]),
    );
    fixture.settle();
    fixture.show_history();

    preview(&mut fixture, 1);
    assert!(fixture.test.take_closed_previews().is_empty());
    preview(&mut fixture, 2);
    assert_eq!(
        fixture.test.take_closed_previews(),
        vec![[1; 32]],
        "picking another revision closes the one before"
    );

    fixture.test.click("workspace.revision.current");
    fixture.settle();
    assert_eq!(
        fixture.test.take_closed_previews(),
        vec![[2; 32]],
        "going back to the current one closes the last preview"
    );
}
