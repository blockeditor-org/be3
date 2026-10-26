use beui::Align;
use beui::TextAlign;
use beui::icons::Glyph;
use beui::icons::{
    ICON_CIRCLE, ICON_CIRCLE_CHECK, ICON_CIRCLE_DASHED, ICON_CIRCLE_DOT_DASHED,
    ICON_CIRCLE_ELLIPSIS, ICON_CIRCLE_X, ICON_FLAG, ICON_OCTAGON_ALERT, ICON_SIGNAL_HIGH,
    ICON_SIGNAL_LOW, ICON_SIGNAL_MEDIUM, ICON_STAR,
};
use beui::reactive::*;
use beui::styled::theme::ThemeKind;
use beui::styled::*;
use beui::{Color32, NodeId};
use database_types::database_schema::{ChoiceColor, ChoiceIcon, DatabaseEnumOption};

pub fn choice_glyph(icon: ChoiceIcon) -> Option<Glyph> {
    Some(match icon {
        ChoiceIcon::None => return None,
        ChoiceIcon::Backlog => ICON_CIRCLE_ELLIPSIS,
        ChoiceIcon::Todo => ICON_CIRCLE,
        ChoiceIcon::Started => ICON_CIRCLE_DOT_DASHED,
        ChoiceIcon::Review => ICON_CIRCLE_DASHED,
        ChoiceIcon::Done => ICON_CIRCLE_CHECK,
        ChoiceIcon::Canceled => ICON_CIRCLE_X,
        ChoiceIcon::Low => ICON_SIGNAL_LOW,
        ChoiceIcon::Medium => ICON_SIGNAL_MEDIUM,
        ChoiceIcon::High => ICON_SIGNAL_HIGH,
        ChoiceIcon::Urgent => ICON_OCTAGON_ALERT,
        ChoiceIcon::Flag => ICON_FLAG,
        ChoiceIcon::Star => ICON_STAR,
    })
}

pub fn choice_icon_label(icon: ChoiceIcon) -> &'static str {
    match icon {
        ChoiceIcon::None => "No icon",
        ChoiceIcon::Backlog => "Backlog",
        ChoiceIcon::Todo => "To do",
        ChoiceIcon::Started => "Started",
        ChoiceIcon::Review => "In review",
        ChoiceIcon::Done => "Done",
        ChoiceIcon::Canceled => "Canceled",
        ChoiceIcon::Low => "Low",
        ChoiceIcon::Medium => "Medium",
        ChoiceIcon::High => "High",
        ChoiceIcon::Urgent => "Urgent",
        ChoiceIcon::Flag => "Flag",
        ChoiceIcon::Star => "Star",
    }
}

pub fn choice_color_label(color: ChoiceColor) -> &'static str {
    match color {
        ChoiceColor::Gray => "Gray",
        ChoiceColor::Blue => "Blue",
        ChoiceColor::Teal => "Teal",
        ChoiceColor::Green => "Green",
        ChoiceColor::Yellow => "Yellow",
        ChoiceColor::Orange => "Orange",
        ChoiceColor::Red => "Red",
        ChoiceColor::Pink => "Pink",
        ChoiceColor::Purple => "Purple",
    }
}

pub fn choice_color(color: ChoiceColor, kind: ThemeKind) -> Color32 {
    let (dark, light) = match color {
        ChoiceColor::Gray => ((141, 153, 174), (96, 106, 124)),
        ChoiceColor::Blue => ((108, 140, 255), (37, 99, 235)),
        ChoiceColor::Teal => ((78, 196, 196), (13, 130, 122)),
        ChoiceColor::Green => ((63, 182, 139), (22, 128, 74)),
        ChoiceColor::Yellow => ((229, 190, 70), (150, 110, 0)),
        ChoiceColor::Orange => ((236, 140, 72), (190, 86, 10)),
        ChoiceColor::Red => ((240, 112, 102), (196, 40, 40)),
        ChoiceColor::Pink => ((232, 122, 172), (186, 40, 118)),
        ChoiceColor::Purple => ((176, 122, 224), (118, 56, 226)),
    };
    let (red, green, blue) = match kind {
        ThemeKind::Dark | ThemeKind::OledDark => dark,
        ThemeKind::Light => light,
        ThemeKind::EPaper => return Color32::from_gray(30),
    };
    Color32::from_rgb(red, green, blue)
}

#[component]
pub fn ChoiceMark(
    option: Prop<Option<DatabaseEnumOption>>,
    #[prop(default = TextSize::Base)] size: TextSize,
) -> NodeId {
    let theme = use_theme();
    let option = create_memo(move || option.get());
    let dot_color = create_memo(move || {
        let kind = theme.kind.get();
        option.with(|option| match option {
            Some(option) => ColorRole::Custom(choice_color(option.color, kind)),
            None => ColorRole::TextMuted,
        })
    });
    let color = create_memo(move || {
        let kind = theme.kind.get();
        option.with(|option| match option {
            Some(option) => TextColor::Custom(choice_color(option.color, kind)),
            None => TextColor::Muted,
        })
    });
    let glyph = create_memo(move || {
        option.with(|option| match option {
            Some(option) => choice_glyph(option.icon),
            None => Some(ICON_CIRCLE),
        })
    });
    let dotted = create_memo(move || glyph.get().is_none());
    let iconic = create_memo(move || !dotted.get());
    let mark = Size::Custom(size.icon_px());
    let dot = Size::Custom(size.icon_px() * 0.5);
    view! {
        <Frame width=mark height=mark>
            <Layers>
                <Show condition={iconic}>
                    <Text size color={color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Show condition={dotted}>
                    <Frame align=Align::Center>
                        <Frame width=dot height=dot radius=Radius::Full fill={dot_color} />
                    </Frame>
                </Show>
            </Layers>
        </Frame>
    }
}
