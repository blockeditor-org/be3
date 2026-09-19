use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_REFRESH};
use block_editor_plugin::beui::reactive::{
    Align, Frame, ItemSize, List, NodeRef, Show, Spacer, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Button, ButtonVariant, Caption, IconButton, TextInput, use_theme,
};
use block_editor_plugin::{Editor, Toolbar};

use super::session::Session;

#[component]
pub fn BrowserTab(editor: Editor) -> NodeId {
    let session = Session::new(&editor);
    let address = session.address();
    let error = session.error();
    let back = session.back();
    let forward = session.forward();

    let no_back = create_memo(clone!(back -> move || back.get().is_none()));
    let no_forward = create_memo(clone!(forward -> move || forward.get().is_none()));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));

    let go_back = clone!(session back -> move || {
        if let Some(index) = back.get_untracked() {
            session.go(index);
        }
    });
    let go_forward = clone!(session forward -> move || {
        if let Some(index) = forward.get_untracked() {
            session.go(index);
        }
    });
    let reload = clone!(session -> move || session.reload());
    let typed = clone!(session -> move |value: String| session.set_address(value));
    let submit = clone!(session -> move |value: String| session.navigate(&value));
    let navigate = clone!(session address -> move || session.navigate(&address.get_untracked()));
    let focus_app = clone!(session -> move |focused: bool| {
        if focused {
            session.focus_app();
        }
    });

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
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
                    <List align=Align::Center spacing=0.0>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Show condition={failed}>
                            <Caption
                                content={reason}
                                color={theme.danger.clone()}
                                @test_id={"browser.error"}
                            />
                        </Show>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                </Frame>
            </List>
        </Frame>
    }
}
