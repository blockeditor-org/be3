//! The bar above a block whose signed log this device doubts (guides/blocks/keys.md, "Forks";
//! `BlockInfo::log_doubt`): what it found, with Details and Dismiss. Writing stays allowed: the
//! bar warns, it does not lock the block.

use be_sdk_beui::beui::TextAlign;
use be_sdk_beui::beui::icons::ICON_TRIANGLE_ALERT;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{LogDoubt, LogDoubtKind, LogDoubtSource};

use super::panel::TabInfo;
use super::workspace::Workspace;
use beui::{Align, Direction};

const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Xs;
const SPACING: Space = Space::Md;

/// What the bar says of `doubt`.
fn saying(doubt: &LogDoubt) -> &'static str {
    match (doubt.kind, doubt.source) {
        (LogDoubtKind::Misbehaved, _) => {
            "The server answered a check of this block's history with a proof that does not hold. It may be showing people different versions."
        }
        (LogDoubtKind::Forked, LogDoubtSource::Server) => {
            "This device was shown a different history of this block earlier. The server may be showing people different versions."
        }
        (LogDoubtKind::Forked, _) => {
            "Another device was shown a different history of this block. The server may be showing people different versions."
        }
    }
}

/// What Details says of `doubt`, seen by account `me`: where the other history came from and
/// how long each is.
fn details(doubt: &LogDoubt, me: [u8; 16]) -> String {
    let whose = match doubt.account {
        Some(account) if account == me => "another of your devices",
        _ => "someone else's device",
    };
    let from = match doubt.source {
        LogDoubtSource::Session => format!("{whose}, open on this block with you"),
        LogDoubtSource::Commit => format!("a version of this block saved on {whose}"),
        LogDoubtSource::Account => "another of your devices".to_owned(),
        LogDoubtSource::Server => "what the server showed this device before".to_owned(),
    };
    format!(
        "Found comparing with {from}. This device holds {} entries of the block's history, the other {}.",
        doubt.ours, doubt.theirs
    )
}

#[component]
pub(crate) fn LogDoubtBar(
    workspace: Workspace,
    info: ReadSignal<Option<TabInfo>>,
) -> DynamicSegment<ListItem> {
    let doubt = create_memo(move || {
        info.with(|info| info.as_ref().and_then(|info| info.log_doubt.clone()))
    });
    let block = create_memo(move || info.with(|info| info.as_ref().map(|info| info.item.id)));
    let open = create_signal(false);
    let said = create_memo(move || {
        doubt.with(|doubt| doubt.as_ref().map(saying).unwrap_or_default().to_owned())
    });
    let detail = create_memo(move || {
        let me = workspace.host().account_id().into_bytes();
        doubt.with(|doubt| {
            doubt
                .as_ref()
                .map(|doubt| details(doubt, me))
                .unwrap_or_default()
        })
    });
    view! {
        <Show value={move || doubt.with(Option::is_some)}>
            <Frame
                fill=ColorRole::Surface
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={"workspace.log-doubt"}
            >
                <List spacing=Space::Xs>
                    <List
                        direction=Direction::Horizontal
                        wrap=true
                        align=Align::Center
                        spacing=SPACING
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text
                                size=TextSize::Xs
                                color=TextColor::Warning
                                align=TextAlign::Center
                            >
                                <Icon glyph=ICON_TRIANGLE_ALERT />
                            </Text>
                            <ListItem shrink=1.0>
                                <Text>{said}</Text>
                            </ListItem>
                        </List>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Button
                                @test_id={"workspace.log-doubt.details"}
                                label={move || match open.get() {
                                    true => "Hide details",
                                    false => "Details",
                                }.to_owned()}
                                variant=ButtonVariant::Secondary
                                on_click={move || open.update(|open| *open = !*open)}
                            />
                            <Button
                                @test_id={"workspace.log-doubt.dismiss"}
                                label="Dismiss"
                                variant=ButtonVariant::Ghost
                                on_click={move || {
                                    if let Some(block) = block.get_untracked() {
                                        workspace.host().dismiss_log_doubt(block);
                                    }
                                }}
                            />
                        </List>
                    </List>
                    <Show value={open}>
                        <Text style=TextStyle::CAPTION @test_id={"workspace.log-doubt.said"}>
                            {detail}
                        </Text>
                    </Show>
                </List>
            </Frame>
        </Show>
    }
}
