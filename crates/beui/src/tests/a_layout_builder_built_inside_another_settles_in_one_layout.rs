use super::*;
use crate::reactive::{
    LayoutBuilder, List, ReadSignal, Show, build, clone, create_effect, create_memo, view,
};

#[test]
fn a_layout_builder_built_inside_another_settles_in_one_layout() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let seen = Rc::clone(&seen);
        move || {
            view! {
                <Frame width=300.0 height=200.0>
                    <LayoutBuilder>
                        {move |size: ReadSignal<Vec2>| {
                            let wide = create_memo(move || size.get().x > 100.0);
                            let seen = Rc::clone(&seen);
                            view! {
                                <Frame height=40.0>
                                    <List spacing=0.0>
                                        <Show condition={wide}>
                                            {move || clone!(seen -> view! {
                                                <LayoutBuilder>
                                                    {move |inner: ReadSignal<Vec2>| {
                                                        let seen = Rc::clone(&seen);
                                                        create_effect(move || seen.borrow_mut().push(inner.get().x));
                                                        view! {
                                                            <Frame height=20.0 />
                                                        }
                                                    }}
                                                </LayoutBuilder>
                                            })}
                                        </Show>
                                    </List>
                                </Frame>
                            }
                        }}
                    </LayoutBuilder>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "the inner builder is reached and built later in the same walk"
    );
    assert_eq!(seen.borrow().last(), Some(&300.0));
}
