use super::*;
use crate::reactive::{ForEach, List, ShowKeepAlive, Text, build, create_signal, view};
use crate::styled::{Button, ButtonVariant};

#[test]
fn a_long_change_is_cut_short_unless_asked_for_in_full() {
    let document = build(|| {
        let (shown, set_shown) = create_signal(false);
        view! {
            <List spacing=0.0>
                <Button
                    label="Toggle"
                    variant=ButtonVariant::Primary
                    on_click={move || set_shown.update(|shown| *shown = !*shown)}
                />
                <ShowKeepAlive condition={shown}>
                    <List spacing=0.0>
                        <ForEach keys={(0..80).collect::<Vec<usize>>()}>
                            {move |row: usize| view! {
                                <Text string={format!("Row {row}")} />
                            }}
                        </ForEach>
                    </List>
                </ShowKeepAlive>
            </List>
        }
    });
    let mut driven = Driven::new(document);

    let changed = driven
        .ask(&["click", "\"Toggle\""])
        .expect("the rows appear");
    assert_eq!(
        changed.lines().count(),
        61,
        "sixty lines and a note:\n{changed}"
    );
    assert!(changed.ends_with("--changes=all prints them, and `drive tree` the whole tree\n"));

    let changed = driven
        .ask(&["--changes=all", "click", "\"Toggle\""])
        .expect("the rows go");
    assert!(changed.lines().count() >= 80, "every change is printed");
    let changed = driven
        .ask(&["--changes=0", "click", "\"Toggle\""])
        .expect("the rows come back");
    assert!(changed.starts_with("... and "), "{changed}");
}
