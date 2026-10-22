use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, path::Path};

use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS, WIN32_ERROR},
        System::Registry::{
            HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW,
        },
        UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW},
    },
    core::PCWSTR,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Noty";

pub fn enable_for_current_user() -> io::Result<()> {
    let executable = std::env::current_exe()?;
    register_at(RUN_KEY, &executable).map(|_| ())
}

pub fn report_failure(error: &io::Error) {
    eprintln!("无法设置 Noty 开机自启: {error}");
    let text = wide(OsStr::new(&format!(
        "无法为当前用户设置开机自启：{error}\nNoty 仍可正常使用，请检查 Windows 启动应用或注册表权限。"
    )));
    let title = wide(OsStr::new("Noty 启动设置"));
    // SAFETY: Both strings remain allocated until the modal dialog closes.
    unsafe {
        if MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        )
        .0 == 0
        {
            eprintln!("无法显示 Noty 开机自启失败提示");
        }
    }
}

fn register_at(key: &str, executable: &Path) -> io::Result<bool> {
    let command = quoted_executable(executable);
    let key = wide(OsStr::new(key));
    let name = wide(OsStr::new(VALUE_NAME));
    let data: Vec<u8> = command.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    // SAFETY: The key and value names are null-terminated; the buffer remains alive
    // throughout each synchronous registry call.
    unsafe {
        let mut size = 0;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        );
        if status == ERROR_SUCCESS {
            let mut previous = vec![0_u8; size as usize];
            let read_status = RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(previous.as_mut_ptr().cast()),
                Some(&mut size),
            );
            if read_status == ERROR_SUCCESS {
                previous.truncate(size as usize);
                if previous == data {
                    return Ok(false);
                }
            } else if read_status != ERROR_MORE_DATA {
                return Err(registry_error(read_status));
            }
        } else if status != ERROR_FILE_NOT_FOUND {
            return Err(registry_error(status));
        }

        let status = RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            data.len()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "启动项路径过长"))?,
        );
        if status != ERROR_SUCCESS {
            return Err(registry_error(status));
        }
    }
    Ok(true)
}

fn quoted_executable(path: &Path) -> Vec<u16> {
    let mut command = vec![b'"' as u16];
    command.extend(path.as_os_str().encode_wide());
    command.push(b'"' as u16);
    command.push(0);
    command
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn registry_error(status: WIN32_ERROR) -> io::Error {
    io::Error::from_raw_os_error(status.0 as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows::Win32::System::Registry::RegDeleteTreeW;

    fn stored_command(key: &str) -> Vec<u16> {
        let key = wide(OsStr::new(key));
        let name = wide(OsStr::new(VALUE_NAME));
        let mut size = 0;
        // SAFETY: The registry buffers remain valid for both synchronous calls.
        unsafe {
            assert_eq!(
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                    RRF_RT_REG_SZ,
                    None,
                    None,
                    Some(&mut size),
                ),
                ERROR_SUCCESS
            );
            let mut data = vec![0_u16; size as usize / 2];
            assert_eq!(
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                    RRF_RT_REG_SZ,
                    None,
                    Some(data.as_mut_ptr().cast()),
                    Some(&mut size),
                ),
                ERROR_SUCCESS
            );
            data.truncate(size as usize / 2);
            data
        }
    }

    #[test]
    fn command_quotes_paths_with_spaces_and_non_ascii_characters() {
        let path = Path::new(r"C:\Program Files\便签工具\Noty.exe");
        let command = OsString::from_wide(&quoted_executable(path));
        assert_eq!(
            command.to_string_lossy(),
            format!("\"{}\"\0", path.display())
        );
    }

    #[test]
    fn registry_entry_is_idempotent_and_updates_when_executable_moves() {
        let key = format!(
            r"Software\Noty\AutostartTest-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        );
        let key_name = wide(OsStr::new(&key));
        let first = Path::new(r"C:\Program Files\Noty\noty.exe");
        let moved = Path::new(r"D:\便签工具\Noty.exe");
        assert!(register_at(&key, first).unwrap());
        assert_eq!(stored_command(&key), quoted_executable(first));
        assert!(!register_at(&key, first).unwrap());
        assert!(register_at(&key, moved).unwrap());
        assert_eq!(stored_command(&key), quoted_executable(moved));
        assert!(!register_at(&key, moved).unwrap());

        // SAFETY: This deletes only the uniquely named test key created above.
        let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(key_name.as_ptr())) };
        assert_eq!(result, ERROR_SUCCESS);
    }
}
