use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame() {
    let (document, [increment, value]) = toolbar_of(|| {
        let count = create_signal(0i64);
        [
            view! {
                <Button on_click={move || count.update(|count| *count += 1)}>
                    <Text>"+"</Text>
                </Button>
            },
            view! {
                <Text>{create_memo(move || count.get().to_string())}</Text>
            },
        ]
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), value), "1");
}
