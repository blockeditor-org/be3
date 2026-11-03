use be_sdk_api::{HostWindow, MenuAsk, Picture};
use beui::reactive::*;
use beui::styled::theme::SCROLL_FADE;
use beui::styled::*;
use beui::{Align, Align2, Direction, NodeId, Sides, Vec2};

use crate::naming::{program_name, window_name};

const SLACK: f32 = 0.5;
const BANNER_PADDING: Space = Space::Lg;
const BANNER_SPACING: Space = Space::Lg;
const NOTICE_PADDING: Space = Space::Md;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowStatus {
    pub name: String,
    pub title: String,
    pub icon: Option<Picture>,
    pub group: String,
    pub raised: u64,
    pub rung: u64,
    pub fullscreen: bool,
    pub responding: bool,
    pub notice: Option<String>,
    pub menu: Option<MenuAsk>,
    pub size: Vec2,
    pub drawn: Vec2,
    pub min_size: Vec2,
}

impl WindowStatus {
    pub fn of(window: &HostWindow, parent: Option<&HostWindow>, drawn: Vec2) -> Self {
        let group = match window.program_id.trim() {
            "" => parent.map(|parent| parent.program_id.clone()),
            _ => None,
        }
        .unwrap_or_else(|| window.program_id.clone());
        Self {
            name: program_name(window, parent),
            title: window_name(window, parent),
            icon: window.icon.as_ref().map(|icon| Picture {
                width: icon.width,
                height: icon.height,
                rgba: icon.rgba.clone(),
            }),
            group,
            raised: window.raised,
            rung: window.rung,
            fullscreen: window.fullscreen.is_some(),
            responding: window.responding,
            notice: window.note.clone(),
            menu: window.menu.map(|menu| MenuAsk {
                asked: menu.asked,
                at: beui::Pos2::ZERO + menu.at,
            }),
            size: window.size,
            drawn,
            min_size: window.min_size,
        }
    }

    pub fn intrinsic(&self) -> Option<Vec2> {
        (self.size.x >= 1.0 && self.size.y >= 1.0).then_some(self.size)
    }
}

pub fn clipped_edges(drawn: Vec2, shown: Vec2) -> Sides {
    let edge = |drawn: f32, shown: f32| match drawn > shown + SLACK {
        true => SCROLL_FADE.min(shown / 2.0).max(0.0),
        false => 0.0,
    };
    Sides {
        left: 0.0,
        top: 0.0,
        right: edge(drawn.x, shown.x),
        bottom: edge(drawn.y, shown.y),
    }
}

pub fn too_small(least: Vec2, shown: Vec2) -> bool {
    least.x > shown.x + SLACK || least.y > shown.y + SLACK
}

#[component]
pub fn WindowBody(
    status: Memo<Option<WindowStatus>>,
    on_force_close: ClickCallback,
    children: Child,
) -> NodeId {
    let shown = create_signal(Vec2::ZERO);
    let fade = create_memo(move || {
        let shown = shown.get();
        status.with(|status| {
            status.as_ref().map_or(Sides::default(), |status| {
                clipped_edges(status.drawn, shown)
            })
        })
    });
    let cramped = create_memo(move || {
        let shown = shown.get();
        status.with(|status| {
            status
                .as_ref()
                .is_some_and(|status| too_small(status.min_size, shown))
        })
    });
    let name = create_memo(move || {
        status.with(|status| {
            status
                .as_ref()
                .map(|status| status.name.clone())
                .unwrap_or_default()
        })
    });
    let responding = create_memo(move || {
        status.with(|status| status.as_ref().is_none_or(|status| status.responding))
    });
    let waited = create_signal(false);
    create_effect(move || {
        if responding.get() {
            waited.set(false);
        }
    });
    let banner = create_memo(move || !responding.get() && !waited.get());
    let cramped_text = create_memo(move || match name.get().is_empty() {
        true => "Too small for this window".to_owned(),
        false => format!("Too small for {}", name.get()),
    });
    let built = view! {
        <Layers>
            <Frame fade_edges={fade}>{children}</Frame>
            <Show condition={cramped}>
                <Frame align=Align2::CENTER>
                    <Frame
                        fill=ColorRole::Surface
                        border={Border::DEFAULT}
                        radius=Radius::Lg
                        padding=NOTICE_PADDING
                        @test_id={"host.window.too_small"}
                    >
                        <Text>{cramped_text}</Text>
                    </Frame>
                </Frame>
            </Show>
            <Show condition={banner}>
                <Frame align={Align2::new(Align::Center, Align::Start)}>
                    <NotResponding name on_wait={move || waited.set(true)} on_force_close />
                </Frame>
            </Show>
        </Layers>
    };
    let measured = measure(built);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            let size = measured.rect.size();
            if shown.get_untracked() != size {
                shown.set(size);
            }
        }
    });
    built
}

#[component]
fn NotResponding(
    name: Memo<String>,
    on_wait: ClickCallback,
    on_force_close: ClickCallback,
) -> NodeId {
    let message = create_memo(move || format!("{} is not responding", name.get()));
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
            corners=CornerSet::BOTTOM
            padding=BANNER_PADDING
            @test_id={"host.window.not_responding"}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BANNER_SPACING>
                <Text>{message}</Text>
                <Button
                    label="Wait"
                    variant=ButtonVariant::Secondary
                    @test_id={"host.window.wait"}
                    on_click={on_wait}
                />
                <Button
                    label="Force close"
                    variant=ButtonVariant::Primary
                    @test_id={"host.window.force_close"}
                    on_click={on_force_close}
                />
            </List>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
