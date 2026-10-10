use super::*;
use crate::reactive::{List, Text, build, create_file_picker, create_signal, view};
use crate::styled::{Button, ButtonVariant};

#[test]
fn a_driver_answers_a_file_dialog_with_a_file() {
    let document = build(|| {
        let (picked, set_picked) = create_signal(String::from("nothing picked"));
        let picker = create_file_picker(move |file| {
            if let Ok(file) = file {
                set_picked.set(format!("{} has {} bytes", file.name, file.data.len()));
            }
        });
        view! {
            <List spacing=8.0>
                <Button
                    label="Open"
                    variant=ButtonVariant::Primary
                    on_click={move || picker.open(FileFilter::new("Notes", &["txt"], &[]))}
                />
                <Text string={picked} />
            </List>
        }
    });
    let mut driven = Driven::headless(document);
    let file = std::env::temp_dir().join("a_driver_answers_a_file_dialog.txt");
    std::fs::write(&file, "hello").expect("the file is written");

    let changed = driven
        .ask(&["upload", &file.display().to_string(), "\"Open\""])
        .expect("the dialog the click opened is answered");
    assert!(
        changed.contains("a_driver_answers_a_file_dialog.txt has 5 bytes"),
        "{changed}"
    );
    assert!(
        driven.ask(&["dismiss"]).is_err(),
        "no dialog is left open to dismiss"
    );
}
