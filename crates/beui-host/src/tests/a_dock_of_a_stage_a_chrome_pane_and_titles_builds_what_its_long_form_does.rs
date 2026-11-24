use super::*;
use crate::dock::{
    DockPane, DockSpec, DockSplit, DockTab, Docking, SIDE_PANEL_WIDTH, SplitSize,
    provide_dock_chrome,
};
use crate::dock_host::unstyled::DockApi;
use crate::styled::{ColorRole, Frame as PanelFrame, Scroll, Space};

/// The short form: a stage, a pane the frame's chrome hides, titles, and nothing else.
fn short(chrome: Signal<bool>, dock: NodeRef) -> Document {
    build_node(move || {
        provide_dock_chrome(chrome.read_only());
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <Docking>
                    <DockSplit>
                        <DockPane stage=true>
                            <DockTab>
                                <Frame @test_id="stage" />
                            </DockTab>
                        </DockPane>
                        <DockPane chrome=true>
                            <DockTab title="Panel">
                                <Text>"Panel content"</Text>
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </dock_host::styled::DockHost>
        }
    })
}

/// The long form spelling out what the short one leaves to the dock: the ids it derives, the side
/// panel's width, the locks, the bare stage and its focus, the frame's chrome and the panel.
fn long(chrome: Signal<bool>, dock: NodeRef) -> Document {
    build_node(move || {
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <Docking focus="stage/">
                    <DockSplit id="0" size={SplitSize::Second(SIDE_PANEL_WIDTH)}>
                        <DockPane id="stage" locked=true bare=true>
                            <DockTab id="stage/">
                                <Frame @test_id="stage" />
                            </DockTab>
                        </DockPane>
                        <Show value={chrome}>
                            <DockPane id="0.1" locked=true>
                                <DockTab id="0.1/Panel" title="Panel">
                                    <PanelFrame fill=ColorRole::Surface>
                                        <Scroll>
                                            <PanelFrame padding=Space::Lg>
                                                <Text>"Panel content"</Text>
                                            </PanelFrame>
                                        </Scroll>
                                    </PanelFrame>
                                </DockTab>
                            </DockPane>
                        </Show>
                    </DockSplit>
                </Docking>
            </dock_host::styled::DockHost>
        }
    })
}

struct Built {
    harness: Harness,
    dock: NodeRef,
    chrome: Signal<bool>,
}

impl Built {
    fn new(build: fn(Signal<bool>, NodeRef) -> Document) -> Self {
        let chrome = create_signal(true);
        let dock = NodeRef::new();
        let mut harness = Harness::sized(build(chrome, dock), WIDE_VIEWPORT);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        Self {
            harness,
            dock,
            chrome,
        }
    }

    fn spec(&self) -> DockSpec {
        self.harness
            .document()
            .api::<DockApi>(self.dock.get())
            .state()
            .spec()
    }

    fn hide_chrome(&mut self) {
        self.chrome.set(false);
        self.harness.frame(Vec::new());
        self.harness.frame(Vec::new());
    }
}

#[test]
fn a_dock_of_a_stage_a_chrome_pane_and_titles_builds_what_its_long_form_does() {
    let _root = crate::reactive::RootScope::new();
    let mut short = Built::new(short);
    let mut long = Built::new(long);

    assert_eq!(
        short.spec(),
        long.spec(),
        "the short form's dock is the long form's"
    );
    assert_eq!(
        short.harness.painting(),
        long.harness.painting(),
        "and paints the same, its panel's body the standard panel"
    );

    short.hide_chrome();
    long.hide_chrome();
    assert_eq!(
        short.spec(),
        long.spec(),
        "hiding the chrome leaves out the chrome pane as the long form's Show does"
    );
    assert_eq!(short.harness.painting(), long.harness.painting());
}
