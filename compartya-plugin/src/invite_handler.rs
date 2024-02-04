use rrplug::{
    create_external_interface,
    mid::utils::{from_char_ptr, to_cstring},
};
use std::ffi::c_char;

use crate::{
    bindings::{CmdSource, EcommandTarget, ENGINE_FUNCTIONS},
    exports::PLUGIN,
    LocalMessage,
};

type JoinHandler = extern "C" fn(*const c_char);

#[allow(unused)]
#[repr(C)]
#[must_use]
pub enum IniviteHandlerResult {
    Sucess,
    Failure,
}

create_external_interface! {
    pub(crate) InviteHandler + InviteHanlderMod => {
        pub fn set_join_handler(handler: JoinHandler) -> ();
        pub fn set_secret(secret: *const c_char) -> IniviteHandlerResult;
        pub fn clear_secret() -> ();
    }
}

pub extern "C" fn compartya_join_handler(lobby_id: *const c_char) {
    let lobby_id: String = unsafe { from_char_ptr(lobby_id) };
    _ = PLUGIN
        .wait()
        .send_runframe
        .lock()
        .send(LocalMessage::ExecuteFunction(Box::new(move || unsafe {
            let cmd = to_cstring(&format!("p_connect_to_lobby {}", lobby_id));

            (ENGINE_FUNCTIONS.wait().cbuf_add_text_type)(
                EcommandTarget::FirstPlayer,
                cmd.as_ptr(),
                CmdSource::Code,
            )
        })));
}
