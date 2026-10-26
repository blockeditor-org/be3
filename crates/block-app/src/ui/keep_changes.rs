use std::time::{Duration, Instant};

use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

const SECOND: Duration = Duration::from_secs(1);
const WIDTH: Size = Size::Custom(400.0);

#[component]
pub(crate) fn KeepChanges(
    open: Prop<bool>,
    title: Prop<String>,
    timeout: Duration,
    #[prop(default = 0)] round: Prop<u64>,
    id: String,
    on_keep: ClickCallback,
    on_revert: ClickCallback,
) -> NodeId {
    let deadline = StoredCell::new(None::<Instant>);
    let left = create_signal(timeout);
    let ticking = create_timer(move || {
        let due = deadline.get()?;
        let remaining = due.saturating_duration_since(now());
        left.set(remaining);
        if remaining.is_zero() {
            deadline.set(None);
            on_revert.call();
            return None;
        }
        Some(until_next_second(remaining))
    });
    let open = create_memo(move || open.get());
    create_effect(move || {
        let opened = open.get();
        round.get();
        untrack(|| match opened {
            true => {
                deadline.set(Some(now() + timeout));
                left.set(timeout);
                ticking.restart(until_next_second(timeout));
            }
            false => {
                deadline.set(None);
                ticking.stop();
            }
        });
    });
    let message = create_memo(move || match left.get().as_secs_f64().ceil() as u64 {
        1 => "Reverting in 1 second.".to_owned(),
        seconds => format!("Reverting in {seconds} seconds."),
    });
    let stop = move || {
        deadline.set(None);
        ticking.stop();
    };
    let keep = move || {
        stop();
        on_keep.call();
    };
    let revert = ClickCallback::new(move || {
        stop();
        on_revert.call();
    });
    let title = create_memo(move || title.get());
    view! {
        <Modal
            open
            motion=ModalMotion::Still
            on_dismiss={revert}
        >|| {
                let countdown_id = format!("{id}.countdown");
                let body_id = id.clone();
                view! {
                    <DialogCard width=WIDTH @test_id={id.clone()}>
                        <DialogBody
                            kind=DialogKind::Alert
                            title
                            id={body_id}
                            actions={vec![
                                DialogAction::primary("Keep", keep),
                                DialogAction::cancel("Revert"),
                            ]}
                            on_dismiss={revert}
                        >
                            <Text style=TextStyle::PARAGRAPH @test_id={countdown_id}>
                                {message}
                            </Text>
                        </DialogBody>
                    </DialogCard>
                }
        }</Modal>
    }
}

fn until_next_second(remaining: Duration) -> Duration {
    match remaining.subsec_nanos() {
        0 => SECOND,
        nanos => Duration::from_nanos(u64::from(nanos)),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
