use be_block::database_schema::{ChoiceColor, ChoiceIcon, DatabaseEnumOption};
use beui::TextAlign;
use beui::icons::{
    ICON_CANCEL, ICON_CHECK_CIRCLE, ICON_FLAG, ICON_INCOMPLETE_CIRCLE, ICON_PENDING, ICON_PRIORITY,
    ICON_RADIO_BUTTON_PARTIAL, ICON_RADIO_BUTTON_UNCHECKED, ICON_SIGNAL_CELLULAR_ALT,
    ICON_SIGNAL_CELLULAR_ALT_1_BAR, ICON_SIGNAL_CELLULAR_ALT_2_BAR, ICON_STAR,
};
use beui::reactive::*;
use beui::styled::theme::ThemeKind;
use beui::styled::*;
use beui::{Color32, NodeId};

pub fn choice_glyph(icon: ChoiceIcon) -> &'static str {
    match icon {
        ChoiceIcon::None => "",
        ChoiceIcon::Backlog => ICON_PENDING,
        ChoiceIcon::Todo => ICON_RADIO_BUTTON_UNCHECKED,
        ChoiceIcon::Started => ICON_RADIO_BUTTON_PARTIAL,
        ChoiceIcon::Review => ICON_INCOMPLETE_CIRCLE,
        ChoiceIcon::Done => ICON_CHECK_CIRCLE,
        ChoiceIcon::Canceled => ICON_CANCEL,
        ChoiceIcon::Low => ICON_SIGNAL_CELLULAR_ALT_1_BAR,
        ChoiceIcon::Medium => ICON_SIGNAL_CELLULAR_ALT_2_BAR,
        ChoiceIcon::High => ICON_SIGNAL_CELLULAR_ALT,
        ChoiceIcon::Urgent => ICON_PRIORITY,
        ChoiceIcon::Flag => ICON_FLAG,
        ChoiceIcon::Star => ICON_STAR,
    }
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
        ThemeKind::EInk => return Color32::from_gray(30),
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
            Some(option) => choice_color(option.color, kind),
            None => theme.text_muted.get(),
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
        option
            .with(|option| match option {
                Some(option) => choice_glyph(option.icon),
                None => ICON_RADIO_BUTTON_UNCHECKED,
            })
            .to_owned()
    });
    let dotted = create_memo(move || glyph.with(String::is_empty));
    let iconic = create_memo(move || !dotted.get());
    let mark = size.icon_px();
    let dot = mark * 0.5;
    view! {
        <Frame width=mark height=mark>
            <Layers>
                <Frame visible={iconic}>
                    <Text size color={color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
                <Frame visible={dotted} align=Align::Center>
                    <Frame
                        width=dot
                        height=dot
                        radius={Radius::custom((dot / 2.0).floor())}
                        fill={ColorRole::custom(dot_color)}
                    />
                </Frame>
            </Layers>
        </Frame>
    }
}
