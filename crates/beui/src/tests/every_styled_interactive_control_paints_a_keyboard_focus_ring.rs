use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn every_styled_interactive_control_paints_a_keyboard_focus_ring() {
    let controls: &[fn() -> NodeId] = &[
        || {
            view! {
                <Button label="Button" variant=styled::ButtonVariant::Primary />
            }
        },
        || {
            view! {
                <Checkbox label="Check" checked=false />
            }
        },
        || {
            view! {
                <Switch on=false />
            }
        },
        || {
            view! {
                <Slider value=0.5 />
            }
        },
        || {
            view! {
                <TextInput value="Text" />
            }
        },
        || {
            view! {
                <Tabs
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                    }}
                    selected="One"
                />
            }
        },
        || {
            view! {
                <RadioGroup
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                    }}
                    selected=Some("One")
                />
            }
        },
        || {
            view! {
                <Listbox
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                    }}
                    selected=Some("One")
                />
            }
        },
        || {
            view! {
                <ToggleButton label="Toggle" pressed=false />
            }
        },
        || {
            view! {
                <Accordion title="Header" open=false>
                    <Text color=Color32::WHITE>"Content"</Text>
                </Accordion>
            }
        },
        || {
            view! {
                <ListRow>
                    <Text color=Color32::WHITE>"Row"</Text>
                </ListRow>
            }
        },
    ];
    for control in controls {
        let (document, [control]) = toolbar_of(|| [control()]);
        let mut harness = Harness::new(document);
        let initial = harness.frame(vec![]);
        let focused = harness.frame(vec![key_event(Key::Tab, true)]);
        let rings = |output: &crate::FrameOutput| {
            output.main().shapes().iter().filter(|shape| matches!(shape,
            crate::Shape::Rect { color, stroke_width, .. } if *color == styled::Theme::DARK.accent && *stroke_width == 2.0
        )).count()
        };
        assert!(
            rings(&focused) > rings(&initial),
            "{} has no focus ring",
            harness.document().node_kind(control)
        );
    }
}
