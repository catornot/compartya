use bindings::{EngineFunctions, HostState, ENGINE_FUNCTIONS};
use compartya_shared::{LobbyUid, Order, Password, PlayerUid};
use parking_lot::Mutex;
use rrplug::{async_call_sq_function, prelude::*};
use std::{
    env,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
};

mod bindings;
mod commands;
mod networking;
mod orders;

pub const MATCHMAKING_SERVER_ADDR: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 2000));

pub enum LocalMessage {
    ExecuteOrder(Order),
    ExecuteFunction(Box<dyn FnOnce() + Send>),
    ConnectToLobby(LobbyUid, Password),
    BecomeHost(Password),
    BecomeUser,
    Leave,
    NewOrder(Order),
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
}

pub struct ComPartyaPlugin {
    recv_runframe: Mutex<Receiver<LocalMessage>>,
    send_runframe: Mutex<Sender<LocalMessage>>,
}

impl Plugin for ComPartyaPlugin {
    fn new(plugin_data: &PluginData) -> Self {
        orders::register_functions(plugin_data);

        let (send_runframe, recv) = mpsc::channel();
        let (send, recv_runframe) = mpsc::channel();

        let args = env::args()
            .zip(env::args().skip(1))
            .filter(|(name, _)| name == "compartya_ip" || name == "compartya_port")
            .collect::<Vec<(String, String)>>();

        const DEFAULT_PORT: &str = ":12352";

        let addr = args
            .iter()
            .find(|(name, _)| name == "compartya_ip")
            .map(|(_, arg)| arg.to_owned())
            .unwrap_or(get_local_ip())
            + &args
                .iter()
                .find(|(name, _)| name == "compartya_port")
                .map(|(_, arg)| ":".to_string() + arg)
                .unwrap_or(DEFAULT_PORT.to_string());

        std::thread::spawn(move || {
            _ = networking::run_connections(recv, send, addr)
                .map_err(|err| _ = log::error!("{err}"))
        });

        Self {
            recv_runframe: Mutex::new(recv_runframe),
            send_runframe: Mutex::new(send_runframe),
        }
    }

    fn on_dll_load(&self, engine_data: Option<&EngineData>, dll_ptr: &DLLPointer) {
        unsafe { EngineFunctions::try_init(dll_ptr, &ENGINE_FUNCTIONS) };

        if let WhichDll::Client = dll_ptr.which_dll() {
            commands::hook_disconnect()
        }

        let Some(engine_data) = engine_data else {
            return;
        };

        commands::create_commands(engine_data)
    }

    fn runframe(&self) {
        let Ok(recved) = self.recv_runframe.lock().try_recv() else {
            return;
        };

        match recved {
            LocalMessage::ExecuteOrder(order) => match order {
                Order::JoinServer(id) => {
                    async_call_sq_function!(ScriptVmType::Ui, "CompartyaJoinServer", id)
                }
                Order::LeaveServer => unsafe {
                    let host_state = ENGINE_FUNCTIONS
                        .wait()
                        .host_state
                        .as_mut()
                        .expect("host state should be valid");

                    host_state.next_state = HostState::HsNewGame;
                    set_c_char_array(&mut host_state.level_name, "mp_lobby");
                },
            },
            LocalMessage::ExecuteFunction(func) => func(),
            _ => {}
        }
    }

    fn on_sqvm_created(&self, sqvm_handle: &CSquirrelVMHandle) {
        orders::init_order_capture(sqvm_handle)
    }
}

fn get_local_ip() -> String {
    let cmd_result = Command::new("ipconfig")
        .output()
        .expect("failed to get ipconfig")
        .stdout;
    String::from_utf8_lossy(&cmd_result)
        .to_string()
        .split('\n')
        .filter(|line| line.contains("  IPv4 Address"))
        .filter_map(|line| line.split(':').nth(1))
        .map(|addr| addr.trim().trim_end())
        .last()
        .expect("couldn't find the machine's ip address")
        .to_string()
}

#[inline]
pub(crate) unsafe fn set_c_char_array<const U: usize>(buf: &mut [std::ffi::c_char; U], new: &str) {
    *buf = [0; U]; // null everything
    buf.iter_mut()
        .zip(new.as_bytes())
        .for_each(|(buf_char, new)| *buf_char = *new as i8);
}
entry!(ComPartyaPlugin);
