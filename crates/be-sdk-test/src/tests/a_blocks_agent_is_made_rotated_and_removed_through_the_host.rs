use super::*;

use crate::HostCall;
use be_sdk_beui::{BlockAgent, Failure};

struct AgentRoot;

#[component]
impl BeuiRoot for AgentRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let status = create_signal("asking".to_owned());
        let host = editor.host();
        editor.then(
            host.create_block_agent(editor.block_id(), "boat VM"),
            move |made: Result<BlockAgent, Failure>| match made {
                Err(failure) => status.set(failure.to_string()),
                Ok(made) => {
                    let agent = Uuid::from_bytes(made.agent);
                    status.set(format!("made {}", made.credentials));
                    editor.then(
                        editor.host().rotate_agent_token(agent),
                        move |rotated: Result<String, Failure>| match rotated {
                            Err(failure) => status.set(failure.to_string()),
                            Ok(token) => {
                                status.set(format!("rotated {token}"));
                                editor.then(
                                    editor.host().remove_agent(agent),
                                    move |removed: Result<(), Failure>| {
                                        status.set(match removed {
                                            Ok(()) => "removed".to_owned(),
                                            Err(failure) => failure.to_string(),
                                        });
                                    },
                                );
                            }
                        },
                    );
                }
            },
        );
        let shown = create_memo(move || status.get());
        view! {
            <Root>
                <Text @test_id={"agent.status"}>{shown}</Text>
            </Root>
        }
    }
}

#[test]
fn a_blocks_agent_is_made_rotated_and_removed_through_the_host() {
    let block = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let mut test = BeuiTest::<AgentRoot>::editing(block);
    test.settle();

    let made = test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::CreateBlockAgent {
                reply,
                block: asked,
                name,
            } if asked == block.into_bytes() && name == "boat VM" => Some(reply),
            _ => None,
        })
        .expect("the editor asks for an agent bound to its block");
    test.answer(
        made,
        Ok(BlockAgent {
            agent: agent.into_bytes(),
            credentials: r#"{"token":"first"}"#.to_owned(),
        }),
    );
    test.settle();
    assert_eq!(test.label("agent.status"), r#"made {"token":"first"}"#);

    let rotated = test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::RotateAgentToken {
                reply,
                agent: asked,
            } if asked == agent.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("the editor rotates the agent's token");
    test.answer(rotated, Ok("second".to_owned()));
    test.settle();
    assert_eq!(test.label("agent.status"), "rotated second");

    let removed = test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::RemoveAgent {
                reply,
                agent: asked,
            } if asked == agent.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("the editor removes the agent");
    test.answer(removed, Err(Failure::Denied));
    test.settle();
    assert_eq!(test.label("agent.status"), Failure::Denied.to_string());
}
