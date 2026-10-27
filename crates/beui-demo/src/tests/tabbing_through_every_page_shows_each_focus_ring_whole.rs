use super::*;
use beui::Key;
use beui::native::overflow::OverflowKind;

const MOST_STOPS: usize = 200;

fn tab(test: &mut DocumentTest) {
    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| {
                Input::Key(beui::Keystroke {
                    key: Some(Key::Tab),
                    pressed,
                    ..Default::default()
                })
            })
            .collect(),
    );
    test.frame(Vec::new());
}

#[test]
fn tabbing_through_every_page_shows_each_focus_ring_whole() {
    let mut cut = Vec::new();
    for page in PAGES {
        let mut test = match page {
            Page::Docking => {
                let mut test = demo(WIDE);
                open(&mut test, page);
                test
            }
            _ => alone(WIDE, page),
        };
        let mut visited = Vec::new();
        for _ in 0..MOST_STOPS {
            tab(&mut test);
            let Some(focused) = test.document().focused_node() else {
                break;
            };
            if visited.contains(&focused) {
                break;
            }
            visited.push(focused);
            cut.extend(
                test.document()
                    .overflows()
                    .into_iter()
                    .filter(|overflow| overflow.kind == OverflowKind::RingCut)
                    .map(|overflow| format!("{}: {overflow}", page.title())),
            );
        }
    }
    cut.dedup();
    assert!(
        cut.is_empty(),
        "{} focus ring(s) are cut off with their control in view; put the padding inside the scroll around its content, or give a scroll inside a padded surface `bleed`:\n  {}",
        cut.len(),
        cut.join("\n  ")
    );
}
