use std::time::Duration;

use beui::Align;
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

const GRACE: Duration = Duration::from_millis(300);

/// What the host answered `asked`, as an async memo reads it: a failure is `Failed`, and a
/// question the host dropped (it closed) is still loading.
pub async fn answer<T>(asked: be_sdk::Asked<Result<T, be_sdk::Failure>>) -> Ready<T> {
    match asked.await {
        Some(Ok(answer)) => Ok(answer),
        Some(Err(failure)) => Err(Pending::Failed(failure.to_string())),
        None => Err(Pending::Loading),
    }
}

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
            <List spacing=Space::None align=Align::Center>
                <Spacer />
                <Show value={late}>
                    <Progress width=INLINE_PROGRESS_WIDTH label={label} />
                </Show>
                <Spacer />
            </List>
        </Frame>
    }
}

#[component]
pub fn FailedPane(error: String) -> NodeId {
    view! {
        <Frame @test_id={"editor.failed"}>
            <List spacing=Space::None align=Align::Center>
                <Spacer />
                <Text color=TextColor::Muted>{error}</Text>
                <Spacer />
            </List>
        </Frame>
    }
}

#[component]
pub fn Loaded(
    #[prop(default = "Loading".to_owned())] label: Prop<String>,
    #[prop(children)] content: Build<Child>,
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
        <Loading fallback>|| { content.call() }</Loading>
    }
}
