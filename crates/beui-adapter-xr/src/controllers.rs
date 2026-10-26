use beui_core::runner::{Controller, Hand, HandPose};
use openxr as xr;

const PROFILES: [(&str, &str, &str); 5] = [
    (
        "/interaction_profiles/khr/simple_controller",
        "select/click",
        "select/click",
    ),
    (
        "/interaction_profiles/valve/index_controller",
        "trigger/value",
        "squeeze/value",
    ),
    (
        "/interaction_profiles/oculus/touch_controller",
        "trigger/value",
        "squeeze/value",
    ),
    (
        "/interaction_profiles/htc/vive_controller",
        "trigger/value",
        "squeeze/click",
    ),
    (
        "/interaction_profiles/microsoft/motion_controller",
        "trigger/value",
        "squeeze/click",
    ),
];

pub(crate) struct Controllers {
    set: xr::ActionSet,
    trigger: xr::Action<f32>,
    squeeze: xr::Action<f32>,
    hands: [Hold; 2],
}

struct Hold {
    hand: Hand,
    path: xr::Path,
    aim: xr::Space,
    grip: xr::Space,
    tracked: bool,
}

fn path(instance: &xr::Instance, path: &str) -> xr::Result<xr::Path> {
    instance.string_to_path(path)
}

impl Controllers {
    pub fn new(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> xr::Result<Self> {
        let set = instance.create_action_set("lasers", "Lasers", 0)?;
        let sides = [Hand::Left, Hand::Right].map(|hand| match hand {
            Hand::Left => "/user/hand/left",
            Hand::Right => "/user/hand/right",
        });
        let paths = [path(instance, sides[0])?, path(instance, sides[1])?];
        let aim = set.create_action::<xr::Posef>("aim", "Aim", &paths)?;
        let grip = set.create_action::<xr::Posef>("grip", "Grip", &paths)?;
        let trigger = set.create_action::<f32>("trigger", "Trigger", &paths)?;
        let squeeze = set.create_action::<f32>("squeeze", "Squeeze", &paths)?;
        for (profile, pull, hold) in PROFILES {
            let mut bindings = Vec::new();
            for side in sides {
                bindings.push(xr::Binding::new(
                    &aim,
                    path(instance, &format!("{side}/input/aim/pose"))?,
                ));
                bindings.push(xr::Binding::new(
                    &grip,
                    path(instance, &format!("{side}/input/grip/pose"))?,
                ));
                bindings.push(xr::Binding::new(
                    &trigger,
                    path(instance, &format!("{side}/input/{pull}"))?,
                ));
                if pull != hold {
                    bindings.push(xr::Binding::new(
                        &squeeze,
                        path(instance, &format!("{side}/input/{hold}"))?,
                    ));
                }
            }
            if let Err(error) =
                instance.suggest_interaction_profile_bindings(path(instance, profile)?, &bindings)
            {
                eprintln!(
                    "beui-adapter-xr: the runtime turned down bindings for {profile}: {error}"
                );
            }
        }
        session.attach_action_sets(&[&set])?;
        let hold = |hand: Hand, path: xr::Path| -> xr::Result<Hold> {
            Ok(Hold {
                hand,
                path,
                aim: aim.create_space(session, path, xr::Posef::IDENTITY)?,
                grip: grip.create_space(session, path, xr::Posef::IDENTITY)?,
                tracked: false,
            })
        };
        Ok(Self {
            hands: [hold(Hand::Left, paths[0])?, hold(Hand::Right, paths[1])?],
            set,
            trigger,
            squeeze,
        })
    }

    pub fn read(
        &mut self,
        session: &xr::Session<xr::Vulkan>,
        space: &xr::Space,
        time: xr::Time,
    ) -> Vec<Controller> {
        let mut out = Vec::new();
        if session
            .sync_actions(&[xr::ActiveActionSet::new(&self.set)])
            .is_err()
        {
            return out;
        }
        for hold in &mut self.hands {
            let hand = hold.hand;
            let aim = located(&hold.aim, space, time);
            match aim {
                Some(aim) => {
                    let grip = located(&hold.grip, space, time).unwrap_or(aim);
                    hold.tracked = true;
                    out.push(Controller::Pose { hand, aim, grip });
                }
                None if std::mem::take(&mut hold.tracked) => {
                    out.push(Controller::Lost { hand });
                    continue;
                }
                None => continue,
            }
            let value = |action: &xr::Action<f32>| {
                action
                    .state(session, hold.path)
                    .ok()
                    .filter(|state| state.is_active)
                    .map_or(0.0, |state| state.current_state.clamp(0.0, 1.0))
            };
            out.push(Controller::Trigger {
                hand,
                value: value(&self.trigger),
            });
            out.push(Controller::Squeeze {
                hand,
                value: value(&self.squeeze),
            });
        }
        out
    }
}

fn located(space: &xr::Space, base: &xr::Space, time: xr::Time) -> Option<HandPose> {
    let location = space.locate(base, time).ok()?;
    let flags = location.location_flags;
    if !flags.contains(
        xr::SpaceLocationFlags::POSITION_VALID | xr::SpaceLocationFlags::ORIENTATION_VALID,
    ) {
        return None;
    }
    let (position, orientation) = (location.pose.position, location.pose.orientation);
    Some(HandPose {
        position: [position.x, position.y, position.z],
        orientation: [orientation.x, orientation.y, orientation.z, orientation.w],
    })
}
