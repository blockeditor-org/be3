use beui::icons::{
    ICON_CIRCLE_ALERT, ICON_REFRESH_CW, ICON_ROTATE_CCW, ICON_ROTATE_CCW_CLOCK, ICON_TRASH,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, NodeId, TextAlign, Wrap};

const PADDING: f32 = 16.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn CrashNotice(
    title: Prop<String>,
    detail: Prop<String>,
    #[prop(default = true)] reloadable: Prop<bool>,
    #[prop(default = "crash-notice".to_owned())] id: String,
    on_reload: ClickCallback,
) -> NodeId {
    let reload_id = format!("{id}.reload");
    let reloadable = create_memo(move || reloadable.get());
    let has_detail = create_memo({
        let detail = detail.clone();
        move || !detail.get().is_empty()
    });
    let detail = create_memo(move || detail.get());
    view! {
        <Frame @test_id={id} padding=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Text size=TextSize::Xl color=TextColor::Danger align=TextAlign::Center>
                    <Icon glyph=ICON_CIRCLE_ALERT />
                </Text>
                <Text style=TextStyle::HEADING align=TextAlign::Center>{title}</Text>
                <Show condition={has_detail}>
                    <Text
                        size=TextSize::Sm
                        color=TextColor::Muted
                        align=TextAlign::Center
                        wrap=Wrap::Words
                    >
                        {detail}
                    </Text>
                </Show>
                <Show condition={reloadable}>|| {
                    let reload_id = reload_id.clone(); view! {
                        <Button
                            @test_id={reload_id}
                            label="Reload"
                            glyph=ICON_REFRESH_CW
                            variant=ButtonVariant::Secondary
                            on_click={on_reload}
                        />
                    }
                }</Show>
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn StaleNotice(
    title: Prop<String>,
    detail: Prop<String>,
    #[prop(default = true)] can_try: Prop<bool>,
    #[prop(default = "stale-notice".to_owned())] id: String,
    on_reset: ClickCallback,
    on_delete: ClickCallback,
    on_try: ClickCallback,
) -> NodeId {
    let (reset_id, delete_id, try_id) = (
        format!("{id}.reset"),
        format!("{id}.delete"),
        format!("{id}.try"),
    );
    view! {
        <Frame @test_id={id} padding=PADDING>
            <List spacing=SPACING align=Align::Center>
                <Text size=TextSize::Xl color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_ROTATE_CCW_CLOCK />
                </Text>
                <Text style=TextStyle::HEADING align=TextAlign::Center>{title}</Text>
                <Text
                    size=TextSize::Sm
                    color=TextColor::Muted
                    align=TextAlign::Center
                    wrap=Wrap::Words
                >
                    {create_memo(move || detail.get())}
                </Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        @test_id={reset_id}
                        label="Reset"
                        glyph=ICON_ROTATE_CCW
                        variant=ButtonVariant::Primary
                        on_click={on_reset}
                    />
                    <Button
                        @test_id={delete_id}
                        label="Delete"
                        glyph=ICON_TRASH
                        variant=ButtonVariant::Secondary
                        on_click={on_delete}
                    />
                    <Show condition={create_memo(move || can_try.get())}>|| {
                        let try_id = try_id.clone(); view! {
                            <Button
                                @test_id={try_id}
                                label="Try anyway"
                                variant=ButtonVariant::Ghost
                                on_click={on_try}
                            />
                        }
                    }</Show>
                </List>
            </List>
        </Frame>
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
