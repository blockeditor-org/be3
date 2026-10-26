use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ARROW_LEFT, ICON_ARROW_RIGHT, ICON_REFRESH_CW};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
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
    view! {
        <List spacing=Space::None>
            <Toolbar shown={chrome}>
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_LEFT
                    label="Back"
                    disabled={no_back}
                    @test_id={"browser.back"}
                    on_click={go_back}
                />
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_RIGHT
                    label="Forward"
                    disabled={no_forward}
                    @test_id={"browser.forward"}
                    on_click={go_forward}
                />
                <Button
                    icon_only=true
                    glyph=ICON_REFRESH_CW
                    label="Reload"
                    @test_id={"browser.reload"}
                    on_click={reload}
                />
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(address, typed)}
                    label="Address"
                    placeholder="Search or enter address"
                    @test_id={"browser.address"}
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
        </List>
    }
}
