use beui::icons::{ICON_KEYBOARD, ICON_MOUSE_POINTER_CLICK};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect, TextAlign};

use super::win32_map::{OFF_SCREEN, SurfaceNotice};
use crate::host::HostItem;
use crate::surfaces::HostItemFace;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hint {
    pub(crate) text: String,
    pub(crate) focused: bool,
}

pub(crate) fn notice_item(notice: &SurfaceNotice) -> HostItem {
    HostItem::Notice {
        text: notice.text(),
        busy: false,
        icon: Some(match notice {
            SurfaceNotice::Elevated => beui::icons::ICON_SHIELD_USER,
            SurfaceNotice::SecureDesktop => beui::icons::ICON_SHIELD_ALERT,
            SurfaceNotice::Focus(_) => beui::icons::ICON_TRIANGLE_ALERT,
        }),
    }
}

#[component]
fn OffScreenPart(part: Memo<Rect>, labelled: bool) -> NodeId {
    let outer_width = create_memo(move || Length::Extent(Extent::fraction(part.get().max.x)));
    let outer_height = create_memo(move || Length::Extent(Extent::fraction(part.get().max.y)));
    let inner = move |min: f32, max: f32| match max > 0.0 {
        true => Length::Extent(Extent::fraction(((max - min) / max).clamp(0.0, 1.0))),
        false => Length::Extent(Extent::fraction(0.0)),
    };
    let width = create_memo(move || {
        let part = part.get();
        inner(part.min.x, part.max.x)
    });
    let height = create_memo(move || {
        let part = part.get();
        inner(part.min.y, part.max.y)
    });
    let label = match labelled {
        true => OFF_SCREEN,
        false => "",
    };
    view! {
        <Frame align=Align2::START>
            <Frame width={outer_width} height={outer_height} align={Align2::new(Align::End, Align::End)}>
                <Frame
                    @test_id="win32.off-screen"
                    width
                    height
                    fill=ColorRole::Scrim
                    align=Align::Center
                    padding={Padding::all(Space::Sm)}
                >
                    <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                        {label}
                    </Text>
                </Frame>
            </Frame>
        </Frame>
    }
}

#[component]
pub(crate) fn EntryOverlay(
    notice: Memo<Option<SurfaceNotice>>,
    hint: Memo<Option<Hint>>,
    off_screen: Memo<Vec<Rect>>,
) -> NodeId {
    let dim = create_memo(move || match notice.get().is_some() {
        true => ColorRole::Dim,
        false => ColorRole::None,
    });
    let keys = create_memo(move || (0..off_screen.with(Vec::len)).collect::<Vec<usize>>());
    let hinted = create_memo(move || hint.with(Option::is_some));
    let text = create_memo(move || hint.with(|hint| hint.as_ref().map(|hint| hint.text.clone())).unwrap_or_default());
    let focused = create_memo(move || hint.with(|hint| hint.as_ref().is_some_and(|hint| hint.focused)));
    let edge = create_memo(move || match focused.get() {
        true => Align2::new(Align::Stretch, Align::Start),
        false => Align2::new(Align::Stretch, Align::End),
    });
    let glyph = create_memo(move || match focused.get() {
        true => Some(ICON_KEYBOARD),
        false => Some(ICON_MOUSE_POINTER_CLICK),
    });
    view! {
        <Layers>
            <ForEach keys={keys}>|index: usize| {
                let part = create_memo(move || {
                    off_screen.with(|parts| parts.get(index).copied().unwrap_or(Rect::ZERO))
                });
                view! {
                    <OffScreenPart part labelled={index == 0} />
                }
            }</ForEach>
            <Frame fill={dim} />
            <Dynamic value={notice}>|notice: Option<SurfaceNotice>| {
                match notice {
                    Some(notice) => {
                        let content = create_memo(move || notice_item(&notice));
                        view! {
                            <Frame align=Align::Center padding={Padding::all(Space::Md)}>
                                <Frame fill=ColorRole::SurfaceRaised border={Border::DEFAULT} radius=Radius::Md>
                                    <HostItemFace content />
                                </Frame>
                            </Frame>
                        }
                    }
                    None => view! {
                        <Frame />
                    },
                }
            }</Dynamic>
            <Show condition={hinted}>
                <Frame align={edge} padding={Padding::all(Space::Md)}>
                    <Frame
                        @test_id="win32.hint"
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius=Radius::Md
                        padding={Padding::all(Space::Md)}
                    >
                        <List direction=Direction::Horizontal spacing=Space::Md align=Align::Center>
                            <Text size=TextSize::Lg color=TextColor::Muted>
                                <Icon glyph={glyph} />
                            </Text>
                            <Text @sizing=ItemSize::Percent(100.0)>{text}</Text>
                        </List>
                    </Frame>
                </Frame>
            </Show>
        </Layers>
    }
}

#[cfg(test)]
mod tests;
