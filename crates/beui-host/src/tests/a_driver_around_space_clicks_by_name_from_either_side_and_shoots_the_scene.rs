use super::*;
use crate::native::drive::{Drive, Inbox, Reply, ask};
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use beui_core::app::SafeArea;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, RunOptions, Runner};
use std::future::Future;
use std::task::Poll;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

#[test]
fn a_driver_around_space_clicks_by_name_from_either_side_and_shoots_the_scene() {
    let document = build_node(|| {
        let pressed = create_signal(false);
        let then = create_signal(false);
        view! {
            <styled::List spacing=Space::Md>
                <Button
                    label="Press me"
                    variant=ButtonVariant::Primary
                    on_click={move || pressed.set(true)}
                />
                <ShowKeepAlive condition={pressed}>
                    <Text>"Pressed"</Text>
                </ShowKeepAlive>
                <Button
                    label="Then me"
                    variant=ButtonVariant::Primary
                    on_click={move || then.set(true)}
                />
                <ShowKeepAlive condition={then}>
                    <Text>"Then pressed"</Text>
                </ShowKeepAlive>
            </styled::List>
        }
    });
    let inbox = Inbox::default();
    let mut taps = Drive::taps(inbox.clone(), &[Some("space"), None]);
    let app = taps.pop().expect("the tap under the app");
    let screen = taps.pop().expect("the tap under space");
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(crate::native::testing::Showing(document)),
        layers: vec![
            Box::new(screen),
            Box::new(beui_layer_space::Space::new()),
            Box::new(app),
        ],
    });
    runner.set_occluded(true);
    let mut window = Simulation::new(
        WindowSize {
            size: vec2(WIDTH as f32, HEIGHT as f32),
            scale: 1.0,
        },
        None,
        runner.reports().clone(),
    );
    let surface = pollster::block_on(beui_renderer_wgpu::offscreen::OffscreenSurface::new(
        WIDTH,
        HEIGHT,
        Default::default(),
    ))
    .expect("no graphics adapter is available");
    runner
        .start(
            Loaded {
                renderer: Box::new(surface),
                fonts: None,
            },
            Setup::new(Waker::new(|| {})),
        )
        .expect("the renderer loads");
    let mut drive = |words: &[&str]| -> Reply {
        let mut answered = std::pin::pin!(ask(
            &inbox,
            words.iter().map(|word| (*word).to_owned()).collect()
        ));
        let mut waiting = std::task::Context::from_waker(std::task::Waker::noop());
        for _ in 0..600 {
            runner.frame(&mut window, SafeArea::default());
            runner.present();
            if let Poll::Ready(reply) = answered.as_mut().poll(&mut waiting) {
                return reply;
            }
        }
        panic!("the app never answered {words:?}");
    };
    let text = |reply: Reply| match reply {
        Reply::Text(text) => text,
        Reply::Error(error) => panic!("{error}"),
        _ => panic!("the answer is not text"),
    };

    text(drive(&["clock", "stop"]));
    let changed = text(drive(&["click", "\"Press me\""]));
    assert!(
        changed.contains("Label value=\"Pressed\""),
        "input reaches the app by default, by name in the app's own tree:\n{changed}"
    );

    let changed = text(drive(&["--into=space", "click", "\"Then me\""]));
    assert!(
        changed.contains("Label value=\"Then pressed\""),
        "into space, the name is carried down to where space shows it, and space's click reaches it:\n{changed}"
    );

    match drive(&["--into=xr", "click", "\"Then me\""]) {
        Reply::Error(error) => assert!(error.contains("space"), "{error}"),
        _ => panic!("--into names only a layer with a tap under it"),
    }
    match drive(&["--into=space", "tree"]) {
        Reply::Error(error) => assert!(error.contains("--into"), "{error}"),
        _ => panic!("reading the app goes nowhere but the app"),
    }

    let Reply::Image { width, .. } = drive(&["shot", "\"Press me\""]) else {
        panic!("a named target is shot where space shows it");
    };
    assert!(
        width > 0 && width < WIDTH,
        "the shot is cropped to the button, not the whole window: {width}"
    );

    let Reply::Image {
        width,
        height,
        rgba,
    } = drive(&["shot"])
    else {
        panic!("a shot is an image");
    };
    assert_eq!(
        (width, height),
        (WIDTH, HEIGHT),
        "the platform window, whole"
    );
    let sky = beui_layer_space::SKY;
    let corner = &rgba[..3];
    assert!(
        corner
            .iter()
            .zip(sky.to_array())
            .all(|(shot, sky)| shot.abs_diff(sky) <= 4),
        "the corner of the shot is space's sky {sky:?}, not the app: {corner:?}"
    );

    assert_eq!(text(drive(&["close"])), "The window closed.\n");
}
