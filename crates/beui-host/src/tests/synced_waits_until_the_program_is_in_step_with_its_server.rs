use super::*;

/// A program behind its server until the network comes back, and then for `frames` more frames.
struct Behind {
    online: bool,
    frames: u32,
}

impl Program for Behind {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if self.online && self.frames > 0 {
            self.frames -= 1;
            context.request_repaint();
        }
    }

    fn lifecycle(&mut self, change: beui_core::program::Lifecycle) {
        if change == beui_core::program::Lifecycle::NetworkChanged {
            self.online = true;
        }
    }

    fn connected(&self) -> Option<bool> {
        Some(self.online)
    }

    fn out_of_step(&self) -> Option<Vec<String>> {
        Some(match (self.online, self.frames) {
            (false, _) => vec!["offline".to_owned()],
            (true, 0) => Vec::new(),
            (true, _) => vec!["catching up".to_owned()],
        })
    }
}

struct Serverless;

impl Program for Serverless {
    fn update(&mut self, _context: &Context, _rect: Rect) {}
}

#[test]
fn synced_waits_until_the_program_is_in_step_with_its_server() {
    let _root = crate::reactive::RootScope::new();
    let mut driven = headless(Behind {
        online: false,
        frames: 3,
    });
    driven
        .ask(&["offline"])
        .expect("offline answers while the program is not connected");
    driven.ask(&["network"]).expect("the network comes back");
    driven
        .ask(&["synced"])
        .expect("synced answers once the program catches up");
    assert_eq!(
        driven.program().frames,
        0,
        "synced waited for the frames the program needed"
    );

    let mut serverless = headless(Serverless);
    assert!(
        serverless.ask(&["synced"]).is_err() && serverless.ask(&["offline"]).is_err(),
        "a program with no server cannot be in step with one, or offline"
    );
}
