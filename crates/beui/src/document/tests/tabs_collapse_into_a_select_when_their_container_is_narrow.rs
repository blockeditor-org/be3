use super::*;
use crate::reactive::{build, view};
use crate::styled::ResponsiveTabs;
use crate::unstyled::Container;

const BREAKPOINT: f32 = 500.0;

#[test]
fn tabs_collapse_into_a_select_when_their_container_is_narrow() {
    let tabs = NodeRef::new();
    let document = build({
        let tabs = tabs.clone();
        move || {
            view! {
                <Container>
                    {move |_| view! {
                        <ResponsiveTabs
                            @node_ref=&tabs
                            labels={vec!["List".to_string(), "Load".to_string(), "Name".to_string()]}
                            selected=1
                            breakpoint=BREAKPOINT
                        />
                    }}
                </Container>
            }
        }
    });
    let tabs = tabs.get();

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let column = tabs;
    let [wide] = harness.document().children(column)[..] else {
        panic!("responsive tabs hold only the branch their width calls for");
    };
    assert!(harness.document().node_rect(wide).is_some());
    assert_eq!(
        styled::responsive_tabs_selected(harness.document(), tabs),
        1
    );

    *harness.viewport_mut() = VIEWPORT;
    harness.frame(Vec::new());

    let [narrow] = harness.document().children(column)[..] else {
        panic!("the narrow branch takes the place of the wide one");
    };
    assert_ne!(
        narrow, wide,
        "a narrow container shows the branch built for it, not the one it replaced"
    );
    assert!(harness.document().node_rect(narrow).is_some());
    assert!(harness.document().node_rect(wide).is_none());
    assert_eq!(styled::select_selected(harness.document(), narrow), Some(1));
}
