use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_REFRESH};
use block_editor_beui::beui::reactive::{
    Align, Frame, ItemSize, Justify, Layers, List, NodeRef, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, IconButton, TextInput, use_theme,
};
use block_editor_beui::{Editor, Subregion, SubregionContent, Toolbar};

use super::session::{PAGE, Session};

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
    let focus_app = move |focused: bool| {
        if focused {
            session.focus_app();
        }
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background}>
            <List spacing=0.0>
                <Toolbar shown={chrome}>
                    <IconButton
                        glyph={ICON_ARROW_BACK.to_owned()}
                        label="Back"
                        disabled={no_back}
                        @test_id={"browser.back"}
                        on_click={go_back}
                    />
                    <IconButton
                        glyph={ICON_ARROW_FORWARD.to_owned()}
                        label="Forward"
                        disabled={no_forward}
                        @test_id={"browser.forward"}
                        on_click={go_forward}
                    />
                    <IconButton
                        glyph={ICON_REFRESH.to_owned()}
                        label="Reload"
                        @test_id={"browser.reload"}
                        on_click={reload}
                    />
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={address}
                        label="Address"
                        placeholder="Search or enter address"
                        @test_id={"browser.address"}
                        on_change={typed}
                        on_submit={submit}
                        on_focus_change={focus_app}
                    />
                    <Button
                        label="Go"
                        variant=ButtonVariant::Primary
                        @test_id={"browser.go"}
                        on_click={navigate}
                    />
                </Toolbar>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <Layers>
                        <Subregion editor={editor} placed={page} punch=false />
                        <List align=Align::Center justify=Justify::Center spacing=0.0>
                            <Show condition={failed}>
                                <Caption
                                    content={reason}
                                    color={theme.danger}
                                    @test_id={"browser.error"}
                                />
                            </Show>
                        </List>
                    </Layers>
                </Frame>
            </List>
        </Frame>
    }
}
