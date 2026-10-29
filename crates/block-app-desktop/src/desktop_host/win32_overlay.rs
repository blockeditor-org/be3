use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Align2, Extent, NodeId, Rect, TextAlign};

use super::win32_map::{OFF_SCREEN, SECURE_DESKTOP};
use crate::host_item::HostItem;
use crate::host_item::HostItemFace;

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
            <Frame
                width={outer_width}
                height={outer_height}
                align={Align2::new(Align::End, Align::End)}
            >
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
pub fn EntryOverlay(secure: Memo<bool>, off_screen: Memo<Vec<Rect>>) -> NodeId {
    let dim = create_memo(move || match secure.get() {
        true => ColorRole::Dim,
        false => ColorRole::None,
    });
    let keys = create_memo(move || (0..off_screen.with(Vec::len)).collect::<Vec<usize>>());
    let content = create_memo(|| HostItem::Notice {
        text: SECURE_DESKTOP.to_owned(),
        busy: false,
        icon: Some(beui::icons::ICON_SHIELD_ALERT),
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
            <Show condition={secure}>
                <Frame align=Align::Center padding={Padding::all(Space::Md)}>
                    <Frame
                        @test_id="win32.secure-desktop"
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius=Radius::Md
                    >
                        <HostItemFace content />
                    </Frame>
                </Frame>
            </Show>
        </Layers>
    }
}

#[cfg(test)]
mod tests;
