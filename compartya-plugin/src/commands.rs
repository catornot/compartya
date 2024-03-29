use rrplug::{
    bindings::cvar::convar::FCVAR_CLIENTDLL, mid::engine::concommands::find_concommand, prelude::*,
};
use std::sync::OnceLock;

use crate::{bindings::ENGINE_FUNCTIONS, exports::PLUGIN, LocalMessage};

static ORIGINAL_DISCONNECT: OnceLock<
    unsafe extern "C" fn(*const rrplug::bindings::cvar::command::CCommand),
> = OnceLock::new();

pub fn hook_disconnect() {
    let disconnect_command = match find_concommand("disconnect") {
        Ok(c) => c,
        Err(err) => {
            return log::error!(
                "couldn't find disconnect command => some features will not work : {err}"
            )
        }
    };

    if let Some(org_func) = disconnect_command
        .m_pCommandCallback
        .replace(disconnect_hook)
    {
        _ = ORIGINAL_DISCONNECT.set(org_func);

        log::info!("replaced disconnect callback");
    }
}

pub fn create_commands(engine_data: &EngineData, token: EngineToken) {
    engine_data
        .register_concommand(
            "p_host_lobby",
            host_lobby,
            "command to start hosting a lobby: p_host_lobby <password;optional>",
            FCVAR_CLIENTDLL as i32,
            token,
        )
        .expect("failed to create host_lobby command");

    engine_data
        .register_concommand(
            "p_connect_to_lobby",
            connect_to_lobby,
            "command to connect to a lobby: p_connect_to_lobby <lobby_id> <password;optional>",
            FCVAR_CLIENTDLL as i32,
            token,
        )
        .expect("failed to create connect_to_lobby command");

    engine_data
        .register_concommand(
            "p_leave",
            leave,
            "command to leave/close a lobby: p_leave",
            FCVAR_CLIENTDLL as i32,
            token,
        )
        .expect("failed to create leave command");

    engine_data
        .register_concommand(
            "p_order_to_this_server",
            order_to_this_server,
            "command to make the party connect to this game",
            FCVAR_CLIENTDLL as i32,
            token,
        )
        .expect("failed to create leave command");

    engine_data
        .register_concommand(
            "p_test_connect",
            test_connect,
            "",
            FCVAR_CLIENTDLL as i32,
            token,
        )
        .expect("failed to create leave command");
}

#[rrplug::concommand]
fn host_lobby(cmd: CCommandResult) -> Option<()> {
    let mut password = cmd
        .get_arg(0)
        .map(|s| s.to_string())
        .unwrap_or_default()
        .chars()
        .collect::<Vec<char>>();
    password.resize(8, ' ');

    if let Err(err) = PLUGIN.wait().send_runframe.send(LocalMessage::BecomeHost(
        password
            .try_into()
            .map_err(|_| log::info!("the password must be 8 chars in lenght"))
            .ok()?,
    )) {
        log::info!("failed to create a new lobby {err}")
    }

    None
}

#[rrplug::concommand]
fn connect_to_lobby(cmd: CCommandResult) -> Option<()> {
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

    if level_name != "mp_lobby" {
        log::error!("you must run this command from mp_lobbby");
        return None;
    }

    let Some(lobby_id) = cmd.get_arg(0).map(|s| s.to_string()) else {
        log::warn!("the lobby id must be 8 chars in lenght");
        return None;
    };
    let mut password = cmd
        .get_arg(1)
        .map(|s| s.to_string())
        .unwrap_or_default()
        .chars()
        .collect::<Vec<char>>();
    password.resize(8, ' ');

    let send_runframe = &PLUGIN.wait().send_runframe;

    if let Err(err) = send_runframe.send(LocalMessage::Leave) {
        log::info!("failed to send downgrade {err}")
    }

    if let Err(err) = send_runframe.send(LocalMessage::ConnectToLobby(
        lobby_id
            .chars()
            .collect::<Vec<char>>()
            .try_into()
            .map_err(|_| log::info!("the lobby id must be 8 chars in lenght"))
            .ok()?,
        password
            .try_into()
            .map_err(|_| log::info!("the password must be 8 chars in lenght"))
            .ok()?,
    )) {
        log::info!("failed to send connection message {err}")
    }

    None
}

#[rrplug::concommand]
fn order_to_this_server() {
    _ = PLUGIN.wait().send_runframe.send(LocalMessage::NewOrder(
        compartya_shared::Order::JoinServer(
            ConVarStruct::find_convar_by_name("ns_server_name", engine_token)
                .ok()
                .map(|cvar| cvar.get_value_string())
                .unwrap_or_default(),
            "".into(),
        ),
    ));
}

#[rrplug::concommand]
fn leave() -> Option<()> {
    if let Err(err) = PLUGIN.wait().send_runframe.send(LocalMessage::Leave) {
        log::info!("failed to leave {err}")
    }

    None
}

#[rrplug::concommand]
fn test_connect() -> Option<()> {
    _ = PLUGIN
        .wait()
        .send_runframe
        .send(LocalMessage::GetCachedOrder);
    None
}

unsafe extern "C" fn disconnect_hook(ccommand: *const rrplug::bindings::cvar::command::CCommand) {
    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .send(LocalMessage::NewOrder(compartya_shared::Order::LeaveServer));

    ORIGINAL_DISCONNECT.get().unwrap()(ccommand);
}
