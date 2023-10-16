#![allow(unused)]

use std::ffi::c_char;

use rrplug::{offset_functions, prelude::*};

#[derive(Debug, Clone)]
#[repr(C)]
pub enum CmdSource {
    // Added to the console buffer by gameplay code.  Generally unrestricted.
    Code,

    // Sent from code via engine->ClientCmd, which is restricted to commands visible
    // via FCVAR_GAMEDLL_FOR_REMOTE_CLIENTS.
    ClientCmd,

    // Typed in at the console or via a user key-bind.  Generally unrestricted, although
    // the client will throttle commands sent to the server this way to 16 per second.
    UserInput,

    // Came in over a net connection as a clc_stringcmd
    // host_client will be valid during this state.
    //
    // Restricted to FCVAR_GAMEDLL commands (but not convars) and special non-ConCommand
    // server commands hardcoded into gameplay code (e.g. "joingame")
    NetClient,

    // Received from the server as the client
    //
    // Restricted to commands with FCVAR_SERVER_CAN_EXECUTE
    NetServer,

    // Being played back from a demo file
    //
    // Not currently restricted by convar flag, but some commands manually ignore calls
    // from this source.  FIXME: Should be heavily restricted as demo commands can come
    // from untrusted sources.
    DemoFile,

    // Invalid value used when cleared
    Invalid = -1,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub enum EcommandTarget {
    CbufFirstPlayer = 0,
    CbufLastPlayer = 1,
    CbufServer = 2,

    CbufCount,
}

#[repr(C)]
pub enum HostState {
    HsNewGame = 0,
    HsLoadGame,
    HsChangeLevelSp,
    HsChangeLevelMp,
    HsRun,
    HsGameShutdown,
    HsShutdown,
    HsRestart,
}

#[repr(C)]
pub struct CHostState {
    pub current_state: HostState,
    pub next_state: HostState,
    pub vec_location: [i32; 3],
    pub ang_location: [i32; 3],
    pub level_name: [c_char; 32],
    pub map_group_name: [c_char; 32],
    pub landmark_name: [c_char; 32],
    pub save_name: [c_char; 32],
    pub short_frame_time: i32, // run a few one-tick frames to avoid large timesteps while loading assets
    pub active_game: bool,
    pub remember_location: bool,
    pub background_level: bool,
    pub waiting_for_connection: bool,
    pub let_tools_override_load_game_ents: bool, // During a load game, this tells Foundry to override ents that are selected in Hammer.
    pub split_screen_connect: bool,
    pub game_has_shut_down_and_flushed_memory: bool, // This is false once we load a map into memory, and set to true once the map is unloaded
    pub workshop_map_download_pending: bool,
}

offset_functions! {
    ENGINE_FUNCTIONS + EngineFunctions for WhichDll::Engine => {
        // ccommand_tokenize = unsafe extern "C" fn(&mut Option<CCommand>, *const c_char, CmdSource) -> bool, at 0x418380;
        cbuf_add_text_type = unsafe extern "C" fn(EcommandTarget, *const c_char, CmdSource) where offset(0x1203B0);
        cbuf_execute = unsafe extern "C" fn() where offset(0x1204B0);
        cbuf_get_current_player = unsafe extern "C" fn() -> EcommandTarget where offset(0x120630);
        // client_array = *const CBaseClient, at 0x12A53F90;
        host_state = *mut CHostState where offset(0x7CF180);
    }
}
