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

pub fn register_functions(plugin_data: &PluginData) {
    plugin_data.register_sq_functions(connected_to_server);
    plugin_data.register_sq_functions(sq_log_error);
    plugin_data.register_sq_functions(sq_log_info);
}

pub fn init_order_capture(handle: &CSquirrelVMHandle) {
    if let ScriptVmType::Client = handle.get_context() {
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
                    .lock()
                    .send(LocalMessage::NewOrder(compartya_shared::Order::LeaveServer));
            }
        }
    }

    let ScriptVmType::Ui = handle.get_context() else {
        return;
    };

    let sqvm = unsafe { handle.get_sqvm() };

    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .lock()
        .send(LocalMessage::ExecuteFunction(Box::new(move || {
            let sqfunctions = SQFUNCTIONS.client.wait();

            _ = compile_string(
                *sqvm.get(),
                sqfunctions,
                true,
                "AddConnectToServerCallback(void function(ServerInfo info) { CompartyaConnectToServerCallback(info) })",
            )
            .map_err(|err| err.log());
        })));
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaConnectToServerCallback")]
fn connected_to_server(server_info: ServerInfo) {
    log::info!("joined server : {}", server_info.name);

    _ = crate::PLUGIN
        .wait()
        .send_runframe
        .lock()
        .send(LocalMessage::NewOrder(compartya_shared::Order::JoinServer(
            server_info.id,
        )));

    Ok(())
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaLogInfo")]
fn sq_log_info(log_msg: String) {
    log::info!("{log_msg}");
    Ok(())
}

#[rrplug::sqfunction(VM = "UI", ExportName = "CompartyaLogError")]
fn sq_log_error(log_msg: String) {
    log::error!("{log_msg}");
    Ok(())
}
