use std::rc::Rc;

use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::TextAlign;
use be_sdk_beui::beui::icons::ICON_GEM;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_type_gem_hunt::SIDE;
use beui::{Align2, Direction};

/// Each cell's side: the size of a button's hit area.
const CELL: Length = Length::Size(Size::Md);

/// What a cell of the field shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Look {
    /// Not dug yet, and nothing known of it; dug by pressing it while `diggable`.
    Hidden { diggable: bool },
    /// Not dug yet, over a gem: shown only to those who may read the world.
    Buried,
    /// Dug up a gem.
    Gem,
    /// Dug, with this many gems around it.
    Near(u8),
}

/// The hunt's field, row by row: `looks` holds what each cell shows, and pressing a cell
/// that may be dug calls `dig` with it.
#[component]
pub fn Field(looks: Memo<Vec<Look>>, dig: Rc<dyn Fn(u16)>) -> NodeId {
    let rows = move |row: u16| {
        let dig = Rc::clone(&dig);
        view! {
            <FieldRow looks row dig />
        }
    };
    view! {
        <List spacing=Space::Xs @test_id={"hunt.field"}>
            <ForEach value={|| (0..SIDE).collect::<Vec<u16>>()} view={rows} />
        </List>
    }
}

#[component]
fn FieldRow(looks: Memo<Vec<Look>>, row: u16, dig: Rc<dyn Fn(u16)>) -> NodeId {
    let cells = move |column: u16| {
        let dig = Rc::clone(&dig);
        let cell = row * SIDE + column;
        view! {
            <Cell looks cell dig />
        }
    };
    view! {
        <List direction=Direction::Horizontal spacing=Space::Xs>
            <ForEach value={|| (0..SIDE).collect::<Vec<u16>>()} view={cells} />
        </List>
    }
}

#[component]
fn Cell(looks: Memo<Vec<Look>>, cell: u16, dig: Rc<dyn Fn(u16)>) -> NodeId {
    let look = create_memo(move || {
        looks.with(|looks| {
            looks
                .get(cell as usize)
                .copied()
                .unwrap_or(Look::Hidden { diggable: false })
        })
    });
    let (row, column) = (cell / SIDE + 1, cell % SIDE + 1);
    // Every cell but a count is a button, so each names what it shows: a gem drawn as an
    // icon has no text of its own to read out.
    let counted = create_memo(move || matches!(look.get(), Look::Near(_)));
    let label = move || match look.get() {
        Look::Hidden { .. } => format!("Dig row {row}, column {column}"),
        Look::Buried => format!("Row {row}, column {column}: a gem lies here"),
        Look::Gem => format!("Row {row}, column {column}: a gem, dug up"),
        Look::Near(_) => String::new(),
    };
    let glyph = move || matches!(look.get(), Look::Gem | Look::Buried).then_some(ICON_GEM);
    let fill = move || match look.get() {
        Look::Gem => ColorRole::Accent,
        Look::Buried => ColorRole::AccentSoft,
        Look::Near(_) => ColorRole::Shade,
        Look::Hidden { .. } => ColorRole::None,
    };
    let near = create_memo(move || match look.get() {
        Look::Near(near) if near > 0 => near.to_string(),
        _ => String::new(),
    });
    let button = move || {
        view! {
            <Button
                icon_only=true
                size=ButtonSize::Small
                label={label}
                glyph={glyph}
                disabled={move || look.get() != Look::Hidden { diggable: true }}
                on_click={pressing(&dig, cell)}
            />
        }
    };
    view! {
        <Frame
            width=CELL
            height=CELL
            align=Align2::CENTER
            fill={fill}
            radius=Radius::Md
            @test_id={format!("hunt.cell.{cell}")}
        >
            <Show value={counted} fallback={button}>
                <Text align=TextAlign::Center>{near}</Text>
            </Show>
        </Frame>
    }
}

/// What pressing `cell` does: digs it.
fn pressing(dig: &Rc<dyn Fn(u16)>, cell: u16) -> impl Fn() + 'static {
    let dig = Rc::clone(dig);
    move || dig(cell)
}
