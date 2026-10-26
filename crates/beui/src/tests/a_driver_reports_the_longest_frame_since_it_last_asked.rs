use super::*;
use crate::FrameTimes;
use crate::primitives::*;
use crate::reactive::*;
use std::time::Duration;

struct Hosting(Document);

impl App for Hosting {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }

    fn other_frames(&mut self) -> Vec<(String, FrameTimes)> {
        vec![(
            "plugin".to_owned(),
            FrameTimes {
                count: 1,
                longest: Duration::from_millis(40),
            },
        )]
    }
}

fn counted(line: &str) -> Option<(String, u64, f64)> {
    let (name, rest) = line.split_once(": ")?;
    let (count, rest) = rest.split_once(" frames, the longest ")?;
    let longest = rest.strip_suffix(" ms")?;
    Some((name.to_owned(), count.parse().ok()?, longest.parse().ok()?))
}

#[test]
fn a_driver_reports_the_longest_frame_since_it_last_asked() {
    let document = build_node(|| {
        view! {
            <Text>"Frames"</Text>
        }
    });
    let mut driven = stepped(Hosting(document));
    driven.ask(&["frames"]).expect("the frames are read");
    driven.ask(&["clock", "stop"]).expect("the clock stops");
    driven.ask(&["pause", "100"]).expect("time passes");

    let frames = driven.ask(&["frames"]).expect("the frames are read");
    let lines: Vec<(String, u64, f64)> = frames.lines().filter_map(counted).collect();
    let [(window, ran, longest), (plugin, stepped, slowest)] = &lines[..] else {
        panic!("frames answers a line for the window and one for the plugin:\n{frames}");
    };
    assert_eq!(
        (window.as_str(), plugin.as_str()),
        ("window", "plugin"),
        "{frames}"
    );
    assert!(
        *ran >= 6,
        "a pause of 100 ms with the clock stopped takes a frame for each 16.7 ms of it:\n{frames}"
    );
    assert!(*longest > 0.0, "{frames}");
    assert_eq!(
        (*stepped, *slowest),
        (*ran, 40.0),
        "what the app ran elsewhere is counted with the window's frames, and its longest kept:\n{frames}"
    );
}
