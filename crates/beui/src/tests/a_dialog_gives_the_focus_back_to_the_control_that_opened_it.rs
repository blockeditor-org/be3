use super::*;
use crate::reactive::{NodeRef, build, create_signal, view};
use crate::styled::{Button, ButtonVariant, Dialog, TextInput};

#[test]
fn a_dialog_gives_the_focus_back_to_the_control_that_opened_it() {
    let (opener, field, go, target) = (
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
    );
    let document = build({
        let (opener, field, go, target) =
            (opener.clone(), field.clone(), go.clone(), target.clone());
        move || {
            let (open, set_open) = create_signal(false);
            let (aimed, set_aimed) = create_signal(false);
            let (opening, closing, aiming) = (set_open.clone(), set_open.clone(), set_open);
            view! {
                <List spacing=8.0>
                    <Button
                        @node_ref=&opener
                        variant=ButtonVariant::Secondary
                        label="Open"
                        on_click={move || opening.set(true)}
                    />
                    <Button
                        @node_ref=&target
                        variant=ButtonVariant::Secondary
                        label="Elsewhere"
                        focused={aimed}
                    />
                    <Dialog
                        open={open.clone()}
                        title="Rename"
                        on_dismiss={move || closing.set(false)}
                    >
                        <List spacing=8.0>
                            <TextInput @node_ref=&field value="" focused={open} />
                            <Button
                                @node_ref=&go
                                variant=ButtonVariant::Primary
                                label="Go elsewhere"
                                on_click={move || {
                                    set_aimed.set(true);
                                    aiming.set(false);
                                }}
                            />
                        </List>
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let open_at = harness.rect(opener.get()).center();
    harness.click(open_at);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(field.get()),
        "the dialog takes the focus into its field"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(opener.get()),
        "closing it gives the focus back to the button that opened it"
    );

    harness.click(open_at);
    harness.frame(Vec::new());
    assert!(harness.document().focus_is_within(field.get()));
    let go_at = harness.rect(go.get()).center();
    harness.click(go_at);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(target.get()),
        "a close that moves the focus on purpose keeps it where it went"
    );
}
