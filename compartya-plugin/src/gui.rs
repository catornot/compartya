use hudhook::{
    hooks::{dx11::ImguiDx11Hooks, ImguiRenderLoop},
    Hudhook,
};
use imgui::*;
use rrplug::{high::UnsafeHandle, prelude::*};
use std::sync::mpsc::{Receiver, Sender};

use crate::LocalMessage;

struct ComPartyaHud {
    should_render: bool,
    sender: Sender<LocalMessage>,
    recv: UnsafeHandle<Receiver<LocalMessage>>, // nah it's safe should be single threaded, locking this is not required
    hosting_lobby: bool,
    lobby_uid: Option<String>,
    party: Vec<String>,
    password: String,
    target_lobby_uid: String,
}

impl ComPartyaHud {
    fn new(sender: Sender<LocalMessage>, recv: Receiver<LocalMessage>) -> Self {
        Self {
            should_render: true,
            sender,
            recv: unsafe { UnsafeHandle::new(recv) },
            hosting_lobby: false,
            party: Vec::new(),
            lobby_uid: None,
            password: String::new(),
            target_lobby_uid: String::new(),
        }
    }
}

impl ImguiRenderLoop for ComPartyaHud {
    fn initialize(&mut self, ctx: &mut Context) {
        imgui_styles::dracula::context_patch(ctx);
    }

    fn render(&mut self, ui: &mut Ui) {
        if let Ok(recved) = self.recv.get_mut().try_recv() {
            match recved {
                LocalMessage::IsHost(hosting) => {
                    self.hosting_lobby = hosting;

                    self.party.clear();
                }
                LocalMessage::LobbyUid(uid) => self.lobby_uid = uid,
                LocalMessage::NewConnection(id) => self.party.push(id),
                LocalMessage::DroppedConnection(id) => {
                    if let Some(index) = self.party.iter().position(|cmp_id| &id == cmp_id) {
                        self.party.swap_remove(index);
                    }
                }
                _ => log::info!("got a unexpected message in gui "),
            }
        }

        ui.window("##partya")
            .collapsed(self.should_render, Condition::FirstUseEver) // change later to hook toggle console and a cmd maybe
            .size([320., 320.], Condition::Always)
            .position([0., 0.], Condition::Always)
            .movable(false)
            .build(|| {
                if let Some(uid) = self.lobby_uid.as_ref() {
                    if self.hosting_lobby {
                        ui.text(&format!("Hosting Lobby: {}", uid));
                    } else {
                        ui.text(&format!("Connected to Party: {}", uid));
                    }
                } else if self.hosting_lobby {
                    ui.text("Hosting Lobby: Without STUN server connection");
                } else {
                    ui.text("Not Connected To Any Party: ");
                }

                if self.hosting_lobby {
                    if ui.button("bring everyone to this server") {
                        _ = self.sender.send(LocalMessage::ForwardToEngine(Box::new(
                            LocalMessage::ExecuteConCommand("p_order_to_this_server".to_string()),
                        )));
                    }
                } else if self.lobby_uid.is_some() {
                    if ui.button("Repeat Order From Host") {
                        _ = self.sender.send(LocalMessage::GetCachedOrder);
                    }
                } else if self.lobby_uid.is_none() {
                    ui.input_text("lobby password", &mut self.password)
                        .chars_noblank(true)
                        .build();

                    ui.input_text("lobby uid", &mut self.target_lobby_uid)
                        .chars_noblank(true)
                        .build();

                    if self.target_lobby_uid.len() < 8 {
                        ui.text("uid is too short");
                    } else if self.target_lobby_uid.len() > 8 {
                        ui.text("uid is too long!");
                    } else if ui.button("connect to lobby") {
                        let mut password = self.password.chars().collect::<Vec<char>>();
                        password.resize(8, ' ');

                        if let Some((uid, password)) = self
                            .target_lobby_uid
                            .chars()
                            .collect::<Vec<char>>()
                            .try_into()
                            .ok()
                            .and_then(|uid| Some((uid, password.try_into().ok()?)))
                        {
                            _ = self
                                .sender
                                .send(LocalMessage::ConnectToLobby(uid, password));

                            self.lobby_uid = Some(self.target_lobby_uid.clone());
                            // just to not send mutitple connect to lobby requests
                        }
                    }

                    if self.password.len() > 8 {
                        ui.text("password is to long!");
                    } else if ui.button("start lobby") {
                        let mut password = self.password.chars().collect::<Vec<char>>();
                        password.resize(8, ' ');

                        if let Ok(password) = password.try_into() {
                            _ = self.sender.send(LocalMessage::BecomeHost(password));

                            self.hosting_lobby = true; // just to not send too many become host messages
                        }
                    }
                }

                if self.lobby_uid.is_none() {
                    return;
                }

                if ui.button("Leave") {
                    _ = self.sender.send(LocalMessage::Leave);
                }

                ui.separator();

                ui.text("Party Members");

                for member in self.party.iter() {
                    ui.text(member);
                }
            });
    }
}

pub fn init_gui(sender: Sender<LocalMessage>, recv: Receiver<LocalMessage>) {
    static mut INIT: bool = false;

    if unsafe { INIT } {
        return;
    }
    unsafe { INIT = true };

    if let Err(e) = Hudhook::builder()
        .with(unsafe { Box::new(ImguiDx11Hooks::new(ComPartyaHud::new(sender, recv))) })
        // .with_hmodule(unsafe { windows::Win32::System::Threading::GetCurrentProcess() })
        .build()
        .apply()
    {
        log::error!("Couldn't apply hooks: {e:?}");
    }
}
