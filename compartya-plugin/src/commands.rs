use rrplug::{
    bindings::cvar::convar::FCVAR_CLIENTDLL, mid::concommands::find_concommand, prelude::*,
};
use std::sync::OnceLock;

use crate::{exports::PLUGIN, LocalMessage};

static ORIGINAL_DISCONNECT: OnceLock<
    unsafe extern "C" fn(*const rrplug::bindings::cvar::command::CCommand),
> = OnceLock::new();

pub fn hook_disconnect() {
    let disconnect_command = match find_concommand("disconnect") {
        Some(c) => c,
        None => return log::error!("couldn't find disconnect command => proxi chat will not work"),
    };

    if let Some(org_func) = disconnect_command
        .m_pCommandCallback
        .replace(disconnect_hook)
    {
        _ = ORIGINAL_DISCONNECT.set(org_func);

        log::info!("replaced disconnect callback");
    }
}

pub fn create_commands(engine_data: &EngineData) {
    engine_data
        .register_concommand(
            "p_host_lobby",
            host_lobby,
            "command to start hosting a lobby: p_host_lobby <password;optional>",
            FCVAR_CLIENTDLL as i32,
        )
        .expect("failed to create host_lobby command");

    engine_data
        .register_concommand(
            "p_connect_to_lobby",
            connect_to_lobby,
            "command to connect to a lobby: p_connect_to_lobby <lobby_id> <password;optional>",
            FCVAR_CLIENTDLL as i32,
        )
        .expect("failed to create connect_to_lobby command");

    engine_data
        .register_concommand(
            "p_leave",
            leave,
            "command to leave/close a lobby: p_leave",
            FCVAR_CLIENTDLL as i32,
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

    if let Err(err) = PLUGIN
        .wait()
        .send_runframe
        .lock()
        .send(LocalMessage::BecomeHost(
            password
                .try_into()
                .map_err(|_| _ = log::info!("the password must be 8 chars in lenght"))
                .ok()?,
        ))
    {
        log::info!("failed to create a new lobby {err}")
    }

    None
}

#[rrplug::concommand]
fn connect_to_lobby(cmd: CCommandResult) -> Option<()> {
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

    let send_runframe = PLUGIN.wait().send_runframe.lock();

    if let Err(err) = send_runframe.send(LocalMessage::Leave) {
        log::info!("failed to send downgrade {err}")
    }

    if let Err(err) = send_runframe.send(LocalMessage::ConnectToLobby(
        lobby_id
            .chars()
            .collect::<Vec<char>>()
            .try_into()
            .map_err(|_| _ = log::info!("the lobby id must be 8 chars in lenght"))
            .ok()?,
        password
            .try_into()
            .map_err(|_| _ = log::info!("the password must be 8 chars in lenght"))
            .ok()?,
    )) {
        log::info!("failed to send connection message {err}")
    }

    None
}

#[rrplug::concommand]
fn leave() -> Option<()> {
    if let Err(err) = PLUGIN.wait().send_runframe.lock().send(LocalMessage::Leave) {
        log::info!("failed to leave {err}")
    }

    None
}

unsafe extern "C" fn disconnect_hook(ccommand: *const rrplug::bindings::cvar::command::CCommand) {
    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .lock()
        .send(LocalMessage::NewOrder(compartya_shared::Order::LeaveServer));

    ORIGINAL_DISCONNECT.get().unwrap()(ccommand);
}
