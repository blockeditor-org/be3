use super::*;
use crate::drive::{Drive, Inbox, Reply, ask};
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::{Launch, RunOptions, Runner};
use std::future::Future;
use std::task::Poll;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

#[test]
fn a_driver_around_space_clicks_by_name_through_its_app_tap_and_shoots_the_scene_through_its_screen_tap()
 {
    let document = build_node(|| {
        let pressed = create_signal(false);
        view! {
            <List spacing=8.0>
                <Button
                    label="Press me"
                    variant=ButtonVariant::Primary
                    on_click={move || pressed.set(true)}
                />
                <ShowKeepAlive condition={pressed}>
                    <Text>"Pressed"</Text>
                </ShowKeepAlive>
            </List>
        }
    });
    let inbox = Inbox::default();
    let (screen, app) = Drive::taps(inbox.clone());
    let mut runner = Runner::new(Launch {
        options: RunOptions::default(),
        context: context(),
        app: Box::new(crate::testing::Showing(document)),
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
        WIDTH, HEIGHT, None,
    ))
    .expect("no graphics adapter is available");
    runner
        .start(
            vec![Loaded {
                renderer: Box::new(surface),
                fonts: None,
            }],
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
    let changed = text(drive(&["app", "click", "\"Press me\""]));
    assert!(
        changed.contains("Label value=\"Pressed\""),
        "the app tap finds the button by name in the app's own tree and clicks it there:\n{changed}"
    );

    match drive(&["app", "shot"]) {
        Reply::Error(error) => assert!(error.contains("drive screen shot"), "{error}"),
        _ => panic!("the app tap takes no shots: a shot is what the platform shows"),
    }
    match drive(&["screen", "click", "\"Press me\""]) {
        Reply::Error(error) => assert!(error.contains("X,Y"), "{error}"),
        _ => panic!("below space, the screen tap cannot find the app's names"),
    }

    let Reply::Image {
        width,
        height,
        rgba,
    } = drive(&["screen", "shot"])
    else {
        panic!("the screen tap's shot is an image");
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
