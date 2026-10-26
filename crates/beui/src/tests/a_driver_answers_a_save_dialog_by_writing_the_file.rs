use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_answers_a_save_dialog_by_writing_the_file() {
    let document = build_node(|| {
        let heard = create_signal(String::from("nothing saved"));
        let export = move || {
            let file = SavedFile {
                name: "notes.txt".to_owned(),
                mime_type: "text/plain".to_owned(),
                data: b"hello".to_vec(),
            };
            save_file(file, move |save| {
                heard.set(match save {
                    FileSave::Saved => "the export was saved".to_owned(),
                    FileSave::Cancelled => "the export was cancelled".to_owned(),
                    FileSave::Failed(error) => format!("the export failed: {error}"),
                });
            });
        };
        view! {
            <List spacing=8.0>
                <Button label="Export" variant=ButtonVariant::Primary on_click=export />
                <Text>{heard}</Text>
            </List>
        }
    });
    let mut driven = headless(crate::testing::Showing(document));
    let directory = std::env::temp_dir().join(format!(
        "a_driver_answers_a_save_dialog.{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("the directory is made");
    let file = directory.join("saved.txt");

    let changed = driven
        .ask(&["screen", "save", &file.display().to_string(), "\"Export\""])
        .expect("the save dialog the click opened is answered");
    assert!(changed.contains("the export was saved"), "{changed}");
    assert_eq!(
        std::fs::read(&file).expect("the file was written"),
        b"hello"
    );
    assert!(
        driven.ask(&["screen", "dismiss"]).is_err(),
        "no dialog is left open to dismiss"
    );

    driven
        .ask(&["app", "click", "\"Export\""])
        .expect("the button opens another save dialog");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(
        state.contains("save dialog open for notes.txt (text/plain, 5 bytes)"),
        "{state}"
    );
    let changed = driven
        .ask(&["screen", "dismiss"])
        .expect("the open save dialog is cancelled");
    assert!(changed.contains("the export was cancelled"), "{changed}");
    let _ = std::fs::remove_dir_all(&directory);
}
