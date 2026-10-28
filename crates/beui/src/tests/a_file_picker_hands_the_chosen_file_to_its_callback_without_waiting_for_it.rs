use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_file_picker_hands_the_chosen_file_to_its_callback_without_waiting_for_it() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal(Vec::new());
    let document = build_node(move || {
        let picker = create_file_picker(move |picked| chosen.update(|chosen| chosen.push(picked)));
        let picking = picker.picking();
        let open = move || picker.open(FileFilter::new("Images", &["png"], &["image/png"]));
        view! {
            <Button @test_id={"choose"} disabled={picking} on_click={open}>
                <Text>"Choose"</Text>
            </Button>
        }
    });
    let mut harness = Harness::new(document);
    let dialogs = |harness: &mut Harness| {
        harness
            .drive(&["state"])
            .lines()
            .filter(|line| line.starts_with("file dialog open for "))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };

    harness.drive(&["click", "#choose"]);
    assert_eq!(dialogs(&mut harness), ["file dialog open for Images (png)"]);
    harness.drive(&["click", "#choose"]);
    assert_eq!(
        dialogs(&mut harness).len(),
        1,
        "a button that is picking asks for nothing more"
    );
    assert!(chosen.with_untracked(|chosen| chosen.is_empty()));

    let directory = std::env::temp_dir().join(format!(
        "a_file_picker_hands_the_chosen_file_to_its_callback.{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("the directory is made");
    let file = directory.join("photo.png");
    std::fs::write(&file, [1, 2, 3]).expect("the file is written");
    harness.drive(&["upload", &file.display().to_string()]);
    let _ = std::fs::remove_dir_all(&directory);
    let photo = PickedFile {
        name: "photo.png".into(),
        data: vec![1, 2, 3],
    };
    assert_eq!(chosen.get_untracked(), [Ok(photo)]);

    harness.drive(&["click", "#choose"]);
    assert_eq!(
        dialogs(&mut harness).len(),
        1,
        "a finished pick lets the button ask again"
    );
    harness.drive(&["dismiss"]);
    assert_eq!(chosen.with_untracked(|chosen| chosen.len()), 1);
    harness.drive(&["click", "#choose"]);
    assert_eq!(dialogs(&mut harness).len(), 1);
}
