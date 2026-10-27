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
use beui::{Color, Color32, ColorDefinitions, ColorVar, NodeId, SchemeColors};
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

beui::color_var!(GRAY, BLUE, TEAL, GREEN, YELLOW, ORANGE, RED, PINK, PURPLE);

fn choice_var(color: ChoiceColor) -> ColorVar {
    match color {
        ChoiceColor::Gray => GRAY,
        ChoiceColor::Blue => BLUE,
        ChoiceColor::Teal => TEAL,
        ChoiceColor::Green => GREEN,
        ChoiceColor::Yellow => YELLOW,
        ChoiceColor::Orange => ORANGE,
        ChoiceColor::Red => RED,
        ChoiceColor::Pink => PINK,
        ChoiceColor::Purple => PURPLE,
    }
}

fn choice_definitions(light: ThemeKind, dark: ThemeKind) -> ColorDefinitions {
    ChoiceColor::ALL
        .into_iter()
        .map(|color| {
            let colors = SchemeColors::new(choice_literal(color, light), choice_literal(color, dark));
            (choice_var(color), colors)
        })
        .collect()
}

#[derive(Clone, Copy)]
pub struct ChoiceColors(ThemeStore);

pub fn use_choice_colors() -> ChoiceColors {
    define_themed_colors(choice_definitions);
    ChoiceColors(use_theme())
}

impl ChoiceColors {
    pub fn get(self, color: ChoiceColor) -> Color {
        self.0
            .themed(choice_var(color), |kind| choice_literal(color, kind))
    }
}

fn choice_literal(color: ChoiceColor, kind: ThemeKind) -> Color32 {
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
    let colors = use_choice_colors();
    let option = create_memo(move || option.get());
    let dot_color = create_memo(move || {
        option.with(|option| match option {
            Some(option) => ColorRole::Custom(colors.get(option.color)),
            None => ColorRole::TextMuted,
        })
    });
    let color = create_memo(move || {
        option.with(|option| match option {
            Some(option) => TextColor::Custom(colors.get(option.color)),
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
