use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{LayerLevel, LayerPolicy, NodeId, OverlayAnchor, Placement, Rect};
use be_sdk_beui::desktop;
use be_sdk_beui::{EndingAnswer, HostWindow, PowerWatch, SessionEnd, WindowsWatch};
use be_sdk_shell::Workspace;
use beui::{Align, Direction, ItemSize};

const MARGIN: Space = Space::Lg;
const PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const SPACING: Space = Space::Md;
const TEXT_SPACING: Space = Space::Xs2;
const NAMED: usize = 3;

fn title(end: SessionEnd) -> &'static str {
    match end {
        SessionEnd::Close => "Closing the desktop",
        SessionEnd::LogOut => "Logging out",
        SessionEnd::Restart => "Restarting",
        SessionEnd::PowerOff => "Powering off",
    }
}

fn now_label(end: SessionEnd) -> &'static str {
    match end {
        SessionEnd::Close => "Close now",
        SessionEnd::LogOut => "Log out now",
        SessionEnd::Restart => "Restart now",
        SessionEnd::PowerOff => "Power off now",
    }
}

pub(crate) fn waiting_for(windows: &[HostWindow]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for window in windows.iter().filter(|window| window.parent.is_none()) {
        let name = match window.name.is_empty() {
            true => window.title.clone(),
            false => window.name.clone(),
        };
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

pub(crate) fn said(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(NAMED).map(String::as_str).collect();
    let listed = match (shown.as_slice(), names.len().saturating_sub(NAMED)) {
        ([], _) => return "Every program has closed.".to_owned(),
        ([one], _) => (*one).to_owned(),
        (_, 0) => format!(
            "{} and {}",
            shown[..shown.len() - 1].join(", "),
            shown[shown.len() - 1]
        ),
        (_, more) => format!("{} and {more} more", shown.join(", ")),
    };
    format!("Waiting for {listed} to close.")
}

pub(crate) fn keep_windows_through_ending(workspace: Workspace) {
    let power = workspace.editor().watch::<PowerWatch>();
    let ending = create_memo(move || power.with(|power| power.ending.is_some()));
    create_effect(move || {
        let ending = ending.get();
        untrack(|| workspace.keep_through_ending(ending));
    });
}

#[component]
pub(crate) fn EndingBar(workspace: Workspace, area: Memo<Rect>) -> NodeId {
    let editor = workspace.editor();
    let power = editor.watch::<PowerWatch>();
    let windows = editor.watch::<WindowsWatch>();
    let ending = create_memo(move || power.with(|power| power.ending));
    let open = create_memo(move || ending.get().is_some());
    let names = create_memo(move || windows.with(|windows| waiting_for(windows)));
    let heading = create_memo(move || ending.get().map(title).unwrap_or_default().to_owned());
    let detail = create_memo(move || names.with(|names| said(names)));
    let now = create_memo(move || ending.get().map(now_label).unwrap_or_default().to_owned());
    let answer = move |answer: EndingAnswer| {
        editor.act(move || desktop::power::answer_ending(answer).detach());
    };
    let anchor = create_memo(move || OverlayAnchor::Rect(area.get()));
    view! {
        <Overlay
            anchor
            placement=Placement::InsideTop
            policy=LayerPolicy::Pass
            level=LayerLevel::Top
            traps_focus=false
            open
            content={move || view! {
                <Frame padding={Padding::all(MARGIN)}>
                    <Frame
                        width=Size::Dialog
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius=Radius::Lg
                        padding=PADDING
                        @test_id={"desktop.ending"}
                    >
                        <List spacing=SPACING>
                            <List spacing=TEXT_SPACING>
                                <Text weight=Weight::Bold>{heading}</Text>
                                <Text size=TextSize::Sm color=TextColor::Muted>{detail}</Text>
                            </List>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=SPACING
                            >
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <Button
                                    label="Cancel"
                                    variant=ButtonVariant::Secondary
                                    @test_id={"desktop.ending.cancel"}
                                    on_click={move || answer(EndingAnswer::Cancel)}
                                />
                                <Button
                                    label={now}
                                    variant=ButtonVariant::Danger
                                    @test_id={"desktop.ending.now"}
                                    on_click={move || answer(EndingAnswer::EndNow)}
                                />
                            </List>
                        </List>
                    </Frame>
                </Frame>
            }}
        />
    }
}
