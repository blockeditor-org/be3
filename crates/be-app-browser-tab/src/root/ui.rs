use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{DockBar, DockPane, DockTab, Docking, DockingLayout};
use be_sdk_beui::beui::icons::{ICON_ARROW_LEFT, ICON_ARROW_RIGHT, ICON_REFRESH_CW};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{Editor, Subregion, SubregionContent};

use super::session::{PAGE, Session};
use beui::{Align, Justify};

const STAGE: &str = "stage";
const TOOLBAR: &str = "toolbar";
const ADDRESS_WIDTH: f32 = 360.0;

#[component]
pub fn BrowserTab(editor: Editor) -> NodeId {
    let session = Session::new(editor);
    let address = session.address();
    let error = session.error();
    let back = session.back();
    let forward = session.forward();

    let no_back = create_memo(move || back.get().is_none());
    let no_forward = create_memo(move || forward.get().is_none());
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let page = create_memo(move || (!failed.get()).then_some(SubregionContent::WebView(PAGE)));

    let go_back = move || {
        if let Some(index) = back.get_untracked() {
            session.go(index);
        }
    };
    let go_forward = move || {
        if let Some(index) = forward.get_untracked() {
            session.go(index);
        }
    };
    let reload = move || session.reload();
    let typed = move |value: String| session.set_address(value);
    let submit = move |value: String| session.navigate(&value);
    let navigate = move || session.navigate(&address.get_untracked());
    let focus_host = move |focused: bool| {
        if focused {
            session.focus_host();
        }
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let bar = Bar {
        address,
        no_back,
        no_forward,
        back: Callback::new(move |()| go_back()),
        forward: Callback::new(move |()| go_forward()),
        reload: Callback::new(move |()| reload()),
        typed: Callback::new(typed),
        submit: Callback::new(submit),
        navigate: Callback::new(move |()| navigate()),
        focus: Callback::new(focus_host),
    };
    view! {
        <List spacing=Space::None>
            <Keyed value={chrome} key={|shown: bool| shown}>|chrome: ReadSignal<bool>| {
                match chrome.get_untracked() {
                    true => view! {
                        <ListItem grow=1.0>
                            <TabDock editor content page failed reason bar />
                        </ListItem>
                    },
                    false => view! {
                        <ListItem grow=1.0>
                            <Page editor content page failed reason />
                        </ListItem>
                    },
                }
            }</Keyed>
        </List>
    }
}

#[derive(Clone, Copy)]
struct Bar {
    address: ReadSignal<String>,
    no_back: Memo<bool>,
    no_forward: Memo<bool>,
    back: Callback<()>,
    forward: Callback<()>,
    reload: Callback<()>,
    typed: Callback<String>,
    submit: Callback<String>,
    navigate: Callback<()>,
    focus: Callback<bool>,
}

#[component]
fn TabDock(
    editor: Editor,
    content: NodeRef,
    page: Memo<Option<SubregionContent>>,
    failed: Memo<bool>,
    reason: Memo<String>,
    bar: Bar,
) -> NodeId {
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockPane id="stage" locked=true bare=true>
                <DockTab id=STAGE title="Page">
                    <Page editor content page failed reason />
                </DockTab>
            </DockPane>
            <DockBar
                id=TOOLBAR
                parent=STAGE
                placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
            >
                <AddressBar bar />
            </DockBar>
        </Docking>
    }
}

#[component]
fn AddressBar(bar: Bar) -> NodeId {
    let Bar {
        address,
        no_back,
        no_forward,
        back,
        forward,
        reload,
        typed,
        submit,
        navigate,
        focus,
    } = bar;
    view! {
        <DockBarSurface spacing=Space::Md @test_id={"browser.toolbar"}>
            <Frame @test_id={"browser.toolbar.grip"}>
                <DockBarGrip />
            </Frame>
            <Button
                icon_only=true
                glyph=ICON_ARROW_LEFT
                label="Back"
                disabled={no_back}
                @test_id={"browser.back"}
                on_click={move || back.call(())}
            />
            <Button
                icon_only=true
                glyph=ICON_ARROW_RIGHT
                label="Forward"
                disabled={no_forward}
                @test_id={"browser.forward"}
                on_click={move || forward.call(())}
            />
            <Button
                icon_only=true
                glyph=ICON_REFRESH_CW
                label="Reload"
                @test_id={"browser.reload"}
                on_click={move || reload.call(())}
            />
            <ListItem length=ADDRESS_WIDTH shrink=1.0>
                <TextInput
                    value={Binding::new(address, move |value: String| typed.call(value))}
                    label="Address"
                    placeholder="Search or enter address"
                    @test_id={"browser.address"}
                    on_submit={move |value: String| submit.call(value)}
                    on_focus_change={move |focused: bool| focus.call(focused)}
                />
            </ListItem>
            <Button
                label="Go"
                variant=ButtonVariant::Primary
                @test_id={"browser.go"}
                on_click={move || navigate.call(())}
            />
        </DockBarSurface>
    }
}

#[component]
fn Page(
    editor: Editor,
    content: NodeRef,
    page: Memo<Option<SubregionContent>>,
    failed: Memo<bool>,
    reason: Memo<String>,
) -> NodeId {
    view! {
        <Frame @node_ref={&content}>
            <Layers>
                <Subregion editor={editor} placed={page} punch=false />
                <List align=Align::Center justify=Justify::Center spacing=Space::None>
                    <Show condition={failed}>
                        <Text
                            style=TextStyle::CAPTION
                            color=TextColor::Danger
                            @test_id={"browser.error"}
                        >
                            {reason}
                        </Text>
                    </Show>
                </List>
            </Layers>
        </Frame>
    }
}
