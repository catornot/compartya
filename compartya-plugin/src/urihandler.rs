use rrplug::prelude::*;
use std::{env, mem};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::System::Registry::{
        RegCreateKeyExW, RegOpenKeyExW, RegSetKeyValueW, HKEY, HKEY_CLASSES_ROOT, KEY_ALL_ACCESS,
        KEY_READ, REG_OPEN_CREATE_OPTIONS, REG_SZ,
    },
};

pub fn try_register_uri_handler() -> Result<(), windows::core::Error> {
    let mut hkey = HKEY(0);

    if unsafe {
        RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            &HSTRING::from("compartya"),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
    } {
        log::info!("URL Handler already exists");
        return Ok(());
    }

    unsafe {
        RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
            &HSTRING::from("compartya\\shell\\open\\command"),
            0,
            PCWSTR::null(),
            REG_OPEN_CREATE_OPTIONS(0),
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        )?;
    }

    let path_to_run = env::current_exe().expect("couldn't get norsthar's path!");

    let command = HSTRING::from(format!("{} %1", path_to_run.display()));
    let flag = HSTRING::from("");
    let name = HSTRING::from("Compartya");

    unsafe {
        RegSetKeyValueW(
            HKEY_CLASSES_ROOT,
            &HSTRING::from("compartya\\shell\\open\\command"),
            &HSTRING::from(""),
            REG_SZ.0,
            Some(command.as_ptr().cast()),
            (command.len() * mem::size_of::<u16>()) as u32,
        )?;
        RegSetKeyValueW(
            HKEY_CLASSES_ROOT,
            &HSTRING::from("compartya"),
            &HSTRING::from("URL Protocol"),
            REG_SZ.0,
            Some(flag.as_ptr().cast()),
            (flag.len() * mem::size_of::<u16>()) as u32,
        )?;
        RegSetKeyValueW(
            HKEY_CLASSES_ROOT,
            &HSTRING::from("compartya"),
            &HSTRING::from(""),
            REG_SZ.0,
            Some(name.as_ptr().cast()),
            (name.len() * mem::size_of::<u16>()) as u32,
        )?;
    }
    Ok(())
}
