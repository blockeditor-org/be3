use super::*;

#[test]
fn a_plugin_hears_every_sample_only_while_it_asks() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    let screen = screens[0].screen;
    instances.screen_set(screens);
    let stroke = || {
        forwarded(
            [20.0, 30.0, 40.0].map(|x| moved(pos2(x, 50.0))).to_vec(),
            Some(pos2(40.0, 50.0)),
            false,
        )
    };
    let xs = |messages: &[Outbound]| -> Vec<f32> {
        input_events(messages)
            .iter()
            .filter_map(|event| match event {
                Input::PointerMoved(pos) => Some(pos.x),
                _ => None,
            })
            .collect()
    };

    let (messages, _) =
        instances.forward(INSTANCE, REGION, &stroke(), &|_, _| beui::Aim::Own, &|_| {
            None
        });
    assert_eq!(
        xs(&messages),
        [30.0],
        "only the latest move of a frame is sent"
    );

    instances.set_frame_reports(vec![block_plugin_api::FrameReport {
        screen,
        content: Rect::ZERO,
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        every_sample: true,
        claims: Vec::new(),
        description: None,
    }]);
    let (messages, _) =
        instances.forward(INSTANCE, REGION, &stroke(), &|_, _| beui::Aim::Own, &|_| {
            None
        });
    assert_eq!(
        xs(&messages),
        [10.0, 20.0, 30.0],
        "every sample is sent while the plugin asks for them"
    );
}
