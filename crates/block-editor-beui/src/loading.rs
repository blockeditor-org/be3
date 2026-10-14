use beui::NodeId;
use beui::reactive::{Align, Frame, ItemSize, List, Prop, Spacer, component, view};
use beui::styled::{Spinner, use_theme};

#[component]
pub fn Loading(#[prop(default = "Loading".to_owned())] label: Prop<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame @test_id={"editor.loading"} color={theme.background}>
            <List spacing=0.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Spinner label={label} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}
