use rrplug::{
    high::{
        squirrel::compile_string,
        squirrel_traits::{GetFromSQObject, GetFromSquirrelVm, PushToSquirrelVm, SQVMName},
    },
    prelude::*,
};

use crate::{bindings::ENGINE_FUNCTIONS, LocalMessage};

#[derive(PushToSquirrelVm, GetFromSQObject, SQVMName, GetFromSquirrelVm)]
struct RequiredModInfo {
    name: String,
    version: String,
}

#[derive(PushToSquirrelVm, SQVMName, GetFromSquirrelVm)]
struct ServerInfo {
    index: i32,
    id: String,
    name: String,
    description: String,
    map: String,
    playlist: String,
    player_count: i32,
    max_player_count: i32,
    requires_password: bool,
    region: String,
    required_mods: Vec<RequiredModInfo>,
}

pub fn register_functions() {
    register_sq_functions(connected_to_server);
    register_sq_functions(sq_log_error);
    register_sq_functions(sq_log_info);
}

pub fn init_order_capture(handle: &CSquirrelVMHandle, _engine_token: EngineToken) {
    if let ScriptContext::CLIENT = handle.get_context() {
        unsafe {
            let host_state = ENGINE_FUNCTIONS
                .wait()
                .host_state
                .as_mut()
                .expect("host state should be valid");

            let level_name = host_state
                .level_name
                .iter()
                .cloned()
                .filter(|i| *i != 0)
                .filter_map(|i| char::from_u32(i as u32))
                .collect::<String>();

            if level_name == "mp_lobby" {
                _ = crate::PLUGIN
                    .wait()
                    .send_runframe
                    .send(LocalMessage::NewOrder(compartya_shared::Order::LeaveServer));
            }
        }
    }

    let ScriptContext::UI = handle.get_context() else {
        return;
    };

    let sqvm = unsafe { handle.get_sqvm() };

    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .send(LocalMessage::ForwardToEngine(Box::new(LocalMessage::ExecuteFunction(Box::new(move || {
            let sqfunctions = SQFUNCTIONS.client.wait();

            _ = compile_string(
                *sqvm.get(),
                sqfunctions,
                true,
                "AddConnectToServerCallback(void function(ServerInfo info) { CompartyaConnectToServerCallback(info) })",
            )
            .map_err(|err| err.log());
        })))));
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaConnectToServerCallback")]
fn connected_to_server(server_info: ServerInfo) {
    log::info!("joined server : {}", server_info.name);

    if server_info.requires_password {
        log::warn!("people won't be able to join your server since it has a password");
        // todo share the password
    }

    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .send(LocalMessage::NewOrder(compartya_shared::Order::JoinServer(
            server_info.id,
            "".into(), // server_info.requires_password.then()
        )));
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaLogInfo")]
fn sq_log_info(log_msg: String) {
    log::info!("{log_msg}");
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaLogError")]
fn sq_log_error(log_msg: String) {
    log::error!("{log_msg}");
}
