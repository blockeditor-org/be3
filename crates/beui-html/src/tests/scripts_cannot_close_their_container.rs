use super::*;

#[test]
#[should_panic(expected = "closing script tag")]
fn scripts_cannot_close_their_container() {
    render_fragment(|| {
        view! {
            <Script kind="module" source="</ScRiPt><p>injected" />
        }
    });
}
