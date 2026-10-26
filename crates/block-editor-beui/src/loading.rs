use std::time::Duration;

use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

const GRACE: Duration = Duration::from_millis(300);

#[component]
pub fn LoadingPane(#[prop(default = "Loading".to_owned())] label: Prop<String>) -> NodeId {
    let label = create_memo(move || label.get());
    let late = create_signal(false);
    let timer = create_timer(move || {
        late.set(true);
        None
    });
    timer.start(GRACE);
    view! {
        <Frame @test_id={"editor.loading"}>
            <List spacing=0.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={late}>
                    <Progress width=INLINE_PROGRESS_WIDTH label={label} />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
pub fn FailedPane(error: String) -> NodeId {
    view! {
        <Frame @test_id={"editor.failed"}>
            <List spacing=0.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Text color=TextColor::Muted>{error}</Text>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
pub fn Loaded(
    #[prop(default = "Loading".to_owned())] label: Prop<String>,
    #[prop(children)] content: Render<()>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let fallback = move |pending: Pending| match pending {
        Pending::Loading => view! {
            <LoadingPane label={label} />
        },
        Pending::Failed(error) => view! {
            <FailedPane error />
        },
    };
    view! {
        <Layers>
            <Loading fallback>|| { content.call(()) }</Loading>
        </Layers>
    }
}
