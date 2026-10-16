use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
pub fn Loading(#[prop(default = "Loading".to_owned())] label: Prop<String>) -> NodeId {
    view! {
        <Frame @test_id={"editor.loading"} fill=ColorRole::Background>
            <List spacing=0.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Spinner label={label} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
pub fn Loaded(
    loaded: Prop<bool>,
    #[prop(default = "Loading".to_owned())] label: Prop<String>,
    #[prop(children)] content: Render<()>,
) -> NodeId {
    let (ready, set_ready) = create_signal(false);
    create_effect(move || {
        if loaded.get() {
            set_ready.set(true);
        }
    });
    let waiting = create_memo(move || !ready.get());
    let shown = create_memo(move || ready.get());
    let label = create_memo(move || label.get());
    view! {
        <Layers>
            <Show condition={waiting}>
                <Loading label={label} />
            </Show>
            <ShowKeepAlive condition={shown}>|| { content.call(()) }</ShowKeepAlive>
        </Layers>
    }
}
