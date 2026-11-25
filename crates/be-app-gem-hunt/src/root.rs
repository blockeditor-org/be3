use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use be_sdk_beui::Loaded;
use be_sdk_beui::MemberEvent;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_gem_hunt::{Hunt, HuntContent};
use uuid::Uuid;

use crate::wire::{Ask, Board, decode, encode};

mod field;
mod players;
mod world;

/// The hunt's editor. One who may read the block sees the whole world, gems included, as
/// its hosts do; one who may only join sees what the host tells it, and digs by asking it.
pub struct HuntRoot;

#[component]
impl be_sdk_beui::BeuiRoot for HuntRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        if editor.joined() {
            view! {
                <Root>
                    <players::Players editor={editor} />
                </Root>
            }
        } else {
            view! {
                <Root>
                    <Loaded>
                        <world::World editor={editor} />
                    </Loaded>
                </Root>
            }
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&HuntContent::new(&Hunt::scattered(seed()))))
    }
}

/// A seed for scattering a world's gems.
pub(crate) fn seed() -> u64 {
    Uuid::new_v4().as_u64_pair().0
}

/// The hunt's host component (guides/apps/host_services.md, "Host components"): the rules.
/// It digs where a player asks, if the cell may be dug, writing the dig to the block, and
/// tells each player the world as that player may see it whenever it changes, and as the
/// player joins.
pub struct HuntHost;

#[component]
impl be_sdk_beui::BeuiRoot for HuntHost {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <be_sdk_beui::beui::styled::Frame />
            </Root>
        }
    }

    fn host(editor: Editor) {
        let hunt = editor.block_content::<HuntContent>();
        let world = hunt.project(|content| content.root());
        // Each member the channel admitted, by its number, with its account.
        let members: Rc<RefCell<BTreeMap<u64, Uuid>>> = Rc::default();
        let tell = move |member: u64, account: Uuid, world: &Hunt| {
            editor.tell_member(member, encode(&Board::of(world, account)));
        };
        {
            let members = Rc::clone(&members);
            create_effect(move || {
                let world = world.get();
                if hunt.ready().is_err() {
                    return;
                }
                for (&member, &account) in members.borrow().iter() {
                    tell(member, account, &world);
                }
            });
        }
        editor.on_member(move |event| match event {
            MemberEvent::Joined(joined) => {
                let account = Uuid::from_bytes(joined.account);
                members.borrow_mut().insert(joined.member, account);
                if hunt.ready().is_ok() {
                    tell(joined.member, account, &world.get_untracked());
                }
            }
            MemberEvent::Said(said) => {
                let Some(&account) = members.borrow().get(&said.member) else {
                    return;
                };
                let Some(Ask::Dig(cell)) = decode(&said.payload) else {
                    return;
                };
                if world.get_untracked().may_dig(cell) {
                    hunt.operate(Hunt::dig(cell, account));
                }
            }
            MemberEvent::Left(member) => {
                members.borrow_mut().remove(&member);
            }
        });
    }
}
