use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_driver_answers_a_file_dialog_in_a_window_that_is_not_simulated() {
    let document = build_node(|| {
        let picked = create_signal(String::from("nothing picked"));
        let picker = create_file_picker(move |file| {
            if let Ok(file) = file {
                picked.set(format!("{} has {} bytes", file.name, file.data.len()));
            }
        });
        view! {
            <List spacing=8.0>
                <Button
                    label="Open"
                    variant=ButtonVariant::Primary
                    on_click={move || picker.open(FileFilter::new("Notes", &["txt"], &[]))}
                />
                <Text>{picked}</Text>
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));
    let directory = std::env::temp_dir().join(format!(
        "a_driver_answers_a_file_dialog_in_a_window.{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("the directory is made");
    let file = directory.join("a_window_file.txt");
    std::fs::write(&file, "hello").expect("the file is written");

    let changed = driven
        .ask(&["upload", &file.display().to_string(), "\"Open\""])
        .expect("the dialog the click opened is answered");
    assert!(
        changed.contains("a_window_file.txt has 5 bytes"),
        "{changed}"
    );
    assert!(
        driven.ask(&["dismiss"]).is_err(),
        "no dialog is left open to dismiss"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
