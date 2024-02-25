use bindings::{EngineFunctions, HostState, ENGINE_FUNCTIONS};
use compartya_shared::{LobbyUid, Order, Password, PlayerUid};
use invite_handler::compartya_join_handler;
use parking_lot::Mutex;
use rrplug::{
    call_sq_function,
    exports::OnceCell,
    high::UnsafeHandle,
    interfaces::external::SourceInterface,
    mid::{squirrel::SQVM_UI, utils::set_c_char_array},
    prelude::*,
};
use std::{
    env,
    net::SocketAddr,
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
};

use crate::invite_handler::InviteHandler;

mod bindings;
mod commands;
mod invite_handler;
mod networking;
mod orders;
mod urihandler;

pub const MATCHMAKING_SERVER_ADDR: &str = env!(
    "MATCHMAKING_SERVER_ADDR",
    "provide the MATCHMAKING_SERVER_ADDR env var for the stun's server address : <ip:port>"
);

pub enum LocalMessage {
    ExecuteOrder(Order),
    ExecuteFunction(Box<dyn FnOnce() + Send>),
    ConnectToLobby(LobbyUid, Password),
    BecomeHost(Password),
    BecomeUser,
    Leave,
    NewOrder(Order),
    GetCachedOrder,
}

#[derive(Debug)]
pub enum ConnectionState {
    User(User),
    Host(Host),
}

#[derive(Default, Debug)]
pub struct Host {
    pub lobby_id: Option<LobbyUid>,
    pub password: Password,
    pub clients: Vec<(SocketAddr, PlayerUid)>,
    pub last_order: Order,
}

#[derive(Default, Debug)]
pub struct User {
    pub server: Option<SocketAddr>,
    pub uid: PlayerUid,
    pub password: Password,
    pub cached_order: Order,
}

pub struct ComPartyaPlugin {
    recv_runframe: Mutex<Receiver<LocalMessage>>,
    send_runframe: Mutex<Sender<LocalMessage>>,
    invite_handler: OnceCell<UnsafeHandle<&'static InviteHandler>>,
}

impl Plugin for ComPartyaPlugin {
    const PLUGIN_INFO: PluginInfo = PluginInfo::new(
        "compartya\0",
        "COMPARTY0\0",
        "COMPARTYA\0",
        PluginContext::CLIENT,
    );

    fn new(_reloaded: bool) -> Self {
        orders::register_functions();

        match urihandler::try_register_uri_handler() {
            Err(err) if err.code() == windows::core::HRESULT::from_win32(0x80070057) => {
                log::warn!(
                "URL Handler can't init itself without running northstar in admin at least once"
            )
            }
            Err(err) => {
                log::error!(
                    "error occrued while trying to register a URL handler : {err:?} : {}",
                    err.code()
                )
            }
            Ok(_) => {}
        }

        let (send_runframe, recv) = mpsc::channel();
        let (send, recv_runframe) = mpsc::channel();

        const IP_STRING: &str = "compartya_ip";
        const PORT_STRING: &str = "compartya_port";

        let args = env::args()
            .zip(env::args().skip(1))
            .filter(|(name, _)| name == IP_STRING || name == PORT_STRING)
            .collect::<Vec<(String, String)>>();

        const DEFAULT_PORT: &str = ":12352";

        log::info!("collected {args:#?}\n real {:#?}", env::args());

        let addr = args
            .iter()
            .find(|(name, _)| name == IP_STRING)
            .map(|(_, arg)| arg.to_owned())
            .unwrap_or_else(get_local_ip)
            + &args
                .iter()
                .find(|(name, _)| name == PORT_STRING)
                .map(|(_, arg)| ":".to_string() + arg)
                .unwrap_or(DEFAULT_PORT.to_string());

        let local_order = env::args().find_map(|arg| {
            arg.starts_with("compartya::%5Copen:")
                .then(|| arg.split_once("open:").map(|server_id| server_id.1))
                .flatten()
                .map(|server_id| Order::JoinServer(server_id.to_string(), String::new()))
        });

        std::thread::spawn(move || {
            _ = networking::run_connections(recv, send, addr, local_order)
                .map_err(|err| log::error!("{err}"))
        });

        Self {
            recv_runframe: Mutex::new(recv_runframe),
            send_runframe: Mutex::new(send_runframe),
            invite_handler: OnceCell::new(),
        }
    }

    fn on_dll_load(
        &self,
        engine_data: Option<&EngineData>,
        dll_ptr: &DLLPointer,
        engine_token: EngineToken,
    ) {
        unsafe { EngineFunctions::try_init(dll_ptr, &ENGINE_FUNCTIONS) };

        if let WhichDll::Client = dll_ptr.which_dll() {
            commands::hook_disconnect()
        }

        let Some(engine_data) = engine_data else {
            return;
        };

        match unsafe { InviteHandler::from_dll_name("DiscordRPC.dll", "InviteHandler001") } {
            Ok(interface) => {
                unsafe { interface.set_join_handler(compartya_join_handler) };

                _ = self
                    .invite_handler
                    .set(unsafe { UnsafeHandle::new(interface) })
            }
            Err(_) => log::warn!("invite handler doesn't exist"),
        }

        commands::create_commands(engine_data, engine_token)
    }

    fn runframe(&self, engine_token: EngineToken) {
        if SQVM_UI.get(engine_token).borrow().is_none() {
            return;
        }

        let Ok(recved) = self.recv_runframe.lock().try_recv() else {
            return;
        };

        let host_state = unsafe {
            ENGINE_FUNCTIONS
                .wait()
                .host_state
                .as_mut()
                .expect("host state should be valid")
        };

        let level_name = host_state
            .level_name
            .iter()
            .cloned()
            .filter(|i| *i != 0)
            .filter_map(|i| char::from_u32(i as u32))
            .collect::<String>();

        match recved {
            LocalMessage::ExecuteOrder(order) => match order {
                Order::JoinServer(id, password) => {
                    _ = call_sq_function!(
                        SQVM_UI.get(engine_token).borrow().expect("should be init"),
                        SQFUNCTIONS.client.wait(),
                        "CompartyaJoinServer",
                        id,
                        password
                    )
                    .map_err(|err| err.log());
                } //compartya::\open:f4bffec013fe65b634ba2ea499a86fa3
                Order::LeaveServer => {
                    if level_name == "mp_lobby" {
                        log::info!("already in mp_lobby");
                        return;
                    }

                    host_state.next_state = HostState::NewGame;
                    set_c_char_array(&mut host_state.level_name, "mp_lobby");
                }
            },
            LocalMessage::ExecuteFunction(func) => func(),
            _ => {}
        }
    }

    fn on_sqvm_created(&self, sqvm_handle: &CSquirrelVMHandle, _engine_token: EngineToken) {
        orders::init_order_capture(sqvm_handle)
    }

    fn on_sqvm_destroyed(&self, _sqvm_handle: &CSquirrelVMHandle, _engine_token: EngineToken) {}
}

fn get_local_ip() -> String {
    let cmd_result = Command::new("ipconfig")
        .output()
        .expect("failed to get ipconfig")
        .stdout;
    String::from_utf8_lossy(&cmd_result)
        .to_string()
        .split('\n')
        .filter(|line| line.contains("IPv4 Address"))
        .filter_map(|line| line.split(':').nth(1))
        .map(|addr| addr.trim().trim_end())
        .last()
        .expect("couldn't find the machine's ip address")
        .to_string()
}

entry!(ComPartyaPlugin);
