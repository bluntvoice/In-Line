//! Windows-only security primitives; no shell commands or network listeners.
use super::contract::McpError;
use std::path::{Path, PathBuf};

pub fn random_secret() -> Result<String, McpError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| McpError::new("security_unavailable"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn root() -> Result<PathBuf, McpError> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("IN_LINE_MCP_TEST_SECURITY_ROOT") {
        return Ok(path.into());
    }
    Ok(dirs::data_local_dir()
        .ok_or_else(|| McpError::new("security_unavailable"))?
        .join("in-line-mcp-security"))
}
#[cfg(windows)]
pub use win::*;

#[cfg(windows)]
mod win {
    use super::*;
    use std::{
        ffi::c_void,
        os::windows::ffi::OsStrExt,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
                ConvertStringSecurityDescriptorToSecurityDescriptorW,
            },
            Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
            GetFileSecurityW, GetSecurityDescriptorControl, GetTokenInformation, SetFileSecurityW,
            TokenGroups, TokenUser, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
            SECURITY_ATTRIBUTES, SE_DACL_PROTECTED, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER,
            UNPROTECTED_DACL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
        System::{
            Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
            SystemServices::SE_GROUP_LOGON_ID,
            Threading::{
                GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    fn unavailable() -> McpError {
        McpError::new("security_unavailable")
    }
    pub fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }
    pub fn user_sid() -> Result<String, McpError> {
        unsafe {
            let mut handle = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 {
                return Err(unavailable());
            }
            let mut size = 0;
            GetTokenInformation(handle, TokenUser, null_mut(), 0, &mut size);
            let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            let success = GetTokenInformation(
                handle,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            );
            CloseHandle(handle);
            if success == 0 {
                return Err(unavailable());
            }
            let token = &*(buffer.as_ptr().cast::<TOKEN_USER>());
            let mut sid = null_mut();
            if ConvertSidToStringSidW(token.User.Sid, &mut sid) == 0 {
                return Err(unavailable());
            }
            let mut len = 0;
            while *sid.add(len) != 0 {
                len += 1;
            }
            let value = String::from_utf16_lossy(std::slice::from_raw_parts(sid, len));
            LocalFree(sid.cast());
            Ok(value)
        }
    }
    pub fn machine_id() -> Result<String, McpError> {
        unsafe {
            let key = wide(std::ffi::OsStr::new("SOFTWARE\\Microsoft\\Cryptography"));
            let name = wide(std::ffi::OsStr::new("MachineGuid"));
            let mut value = [0u16; 128];
            let mut size = std::mem::size_of_val(&value) as u32;
            if RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                value.as_mut_ptr().cast(),
                &mut size,
            ) != 0
            {
                return Err(unavailable());
            }
            Ok(String::from_utf16_lossy(
                &value[..value.iter().position(|x| *x == 0).ok_or_else(unavailable)?],
            ))
        }
    }
    pub fn logon_sid() -> Result<String, McpError> {
        unsafe {
            let mut handle = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) == 0 {
                return Err(unavailable());
            }
            let mut size = 0;
            GetTokenInformation(handle, TokenGroups, null_mut(), 0, &mut size);
            let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            let ok = GetTokenInformation(
                handle,
                TokenGroups,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            );
            CloseHandle(handle);
            if ok == 0 {
                return Err(unavailable());
            }
            let groups = &*buffer.as_ptr().cast::<TOKEN_GROUPS>();
            for group in
                std::slice::from_raw_parts(groups.Groups.as_ptr(), groups.GroupCount as usize)
            {
                if group.Attributes & (SE_GROUP_LOGON_ID as u32) == (SE_GROUP_LOGON_ID as u32) {
                    let mut sid = null_mut();
                    if ConvertSidToStringSidW(group.Sid, &mut sid) == 0 {
                        return Err(unavailable());
                    }
                    let mut len = 0;
                    while *sid.add(len) != 0 {
                        len += 1;
                    }
                    let value = String::from_utf16_lossy(std::slice::from_raw_parts(sid, len));
                    LocalFree(sid.cast());
                    return Ok(value);
                }
            }
            Err(unavailable())
        }
    }
    pub fn verify_server(handle: *mut c_void) -> Result<(), McpError> {
        let expected = std::env::current_exe()
            .map_err(|_| unavailable())?
            .with_file_name("in-line.exe");
        #[cfg(debug_assertions)]
        let expected = std::env::var_os("IN_LINE_MCP_TEST_HOST_EXE")
            .map(PathBuf::from)
            .unwrap_or(expected);
        check_path(&expected)?;
        unsafe {
            let mut pid = 0;
            if windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId(handle, &mut pid) == 0
            {
                return Err(unavailable());
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return Err(unavailable());
            }
            let mut path = vec![0u16; 32768];
            let mut length = path.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
            CloseHandle(process);
            if ok == 0 {
                return Err(unavailable());
            }
            let actual = PathBuf::from(String::from_utf16_lossy(&path[..length as usize]));
            check_path(&actual)?;
            let normalize = |path: &Path| -> Result<String, McpError> {
                Ok(std::fs::canonicalize(path)
                    .map_err(|_| McpError::new("host_unavailable"))?
                    .to_string_lossy()
                    .to_lowercase())
            };
            if normalize(&actual)? != normalize(&expected)? {
                return Err(McpError::new("host_unavailable"));
            }
            Ok(())
        }
    }
    pub fn host_already_running() -> bool {
        #[cfg(debug_assertions)]
        if std::env::var_os("IN_LINE_MCP_TEST_DATA_ROOT").is_some() {
            return false;
        }
        use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return true;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry);
            let mut found = false;
            while more != 0 {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|x| *x == 0)
                    .unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..end])
                    .eq_ignore_ascii_case("in-line.exe")
                {
                    found = true;
                    break;
                }
                more = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
            found
        }
    }
    pub struct Descriptor(*mut c_void, bool);
    impl Descriptor {
        pub fn capture(path: &Path) -> Result<Self, McpError> {
            check_path(path)?;
            let path = wide(path.as_os_str());
            unsafe {
                let mut size = 0;
                GetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    0,
                    &mut size,
                );
                let mut bytes =
                    vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
                if GetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    bytes.as_mut_ptr().cast(),
                    size,
                    &mut size,
                ) == 0
                {
                    return Err(unavailable());
                }
                let mut text = null_mut();
                let mut length = 0;
                if ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    bytes.as_mut_ptr().cast(),
                    1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    &mut length,
                ) == 0
                {
                    return Err(unavailable());
                }
                let mut sd = null_mut();
                let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text,
                    1,
                    &mut sd,
                    null_mut(),
                );
                LocalFree(text.cast());
                if ok == 0 {
                    return Err(unavailable());
                }
                let mut control = 0;
                let mut revision = 0;
                if GetSecurityDescriptorControl(
                    bytes.as_mut_ptr().cast(),
                    &mut control,
                    &mut revision,
                ) == 0
                {
                    LocalFree(sd.cast());
                    return Err(unavailable());
                }
                Ok(Self(sd, control & SE_DACL_PROTECTED != 0))
            }
        }
        pub fn for_pipe() -> Result<Self, McpError> {
            let value = wide(std::ffi::OsStr::new(&format!(
                "D:P(A;;FA;;;{})",
                logon_sid()?
            )));
            let mut sd = null_mut();
            if unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    value.as_ptr(),
                    1,
                    &mut sd,
                    null_mut(),
                )
            } == 0
            {
                return Err(unavailable());
            }
            Ok(Self(sd, true))
        }
        pub fn new(inherit: bool) -> Result<Self, McpError> {
            let sddl = format!(
                "D:P(A;{};FA;;;{})",
                if inherit { "OICI" } else { "" },
                user_sid()?
            );
            let value = wide(std::ffi::OsStr::new(&sddl));
            let mut sd = null_mut();
            if unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    value.as_ptr(),
                    1,
                    &mut sd,
                    null_mut(),
                )
            } == 0
            {
                return Err(unavailable());
            }
            Ok(Self(sd, true))
        }
        pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.0,
                bInheritHandle: 0,
            }
        }
        pub fn apply(&self, path: &Path) -> Result<(), McpError> {
            let path = wide(path.as_os_str());
            if unsafe {
                SetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION
                        | if self.1 {
                            PROTECTED_DACL_SECURITY_INFORMATION
                        } else {
                            UNPROTECTED_DACL_SECURITY_INFORMATION
                        },
                    self.0,
                )
            } == 0
            {
                return Err(unavailable());
            }
            Ok(())
        }
    }
    impl Descriptor {
        pub fn verify(&self, path: &Path) -> Result<(), McpError> {
            let path = wide(path.as_os_str());
            unsafe {
                let mut size = 0;
                GetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    0,
                    &mut size,
                );
                let mut bytes =
                    vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
                if GetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    bytes.as_mut_ptr().cast(),
                    size,
                    &mut size,
                ) == 0
                {
                    return Err(unavailable());
                }
                let stringify = |sd: *mut c_void| -> Result<String, McpError> {
                    let mut value = null_mut();
                    let mut length = 0;
                    if ConvertSecurityDescriptorToStringSecurityDescriptorW(
                        sd,
                        1,
                        DACL_SECURITY_INFORMATION,
                        &mut value,
                        &mut length,
                    ) == 0
                    {
                        return Err(unavailable());
                    }
                    let result = String::from_utf16_lossy(std::slice::from_raw_parts(
                        value,
                        length.saturating_sub(1) as usize,
                    ));
                    LocalFree(value.cast());
                    Ok(result)
                };
                if stringify(bytes.as_mut_ptr().cast())? != stringify(self.0)? {
                    return Err(unavailable());
                }
                Ok(())
            }
        }
    }
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    pub struct HostGate {
        mutex: *mut c_void,
        ready: *mut c_void,
        owner: bool,
    }
    // Kernel handles remain valid while Arc<HostGate> is held. Mutex release stays on run's thread.
    unsafe impl Send for HostGate {}
    unsafe impl Sync for HostGate {}
    impl HostGate {
        pub fn enter(name: &str) -> Result<Self, McpError> {
            use windows_sys::Win32::System::Threading::*;
            let descriptor = Descriptor::new(false)?;
            let attrs = descriptor.attributes();
            let sid = user_sid()?;
            let mutex_name = wide(std::ffi::OsStr::new(&format!(
                "Global\\InLine-host-{sid}-{name}"
            )));
            let event_name = wide(std::ffi::OsStr::new(&format!(
                "Global\\InLine-ready-{sid}-{name}"
            )));
            unsafe {
                let mutex = CreateMutexW(&attrs, 0, mutex_name.as_ptr());
                if mutex.is_null() {
                    return Err(unavailable());
                }
                let ready = CreateEventW(&attrs, 1, 0, event_name.as_ptr());
                if ready.is_null() {
                    CloseHandle(mutex);
                    return Err(unavailable());
                }
                let acquired = WaitForSingleObject(mutex, 0);
                let owner = acquired == 0 || acquired == 0x80;
                if owner {
                    ResetEvent(ready);
                } else if WaitForSingleObject(ready, 15_000) != 0 {
                    CloseHandle(mutex);
                    CloseHandle(ready);
                    return Err(McpError::new("host_unavailable"));
                }
                Ok(Self {
                    mutex,
                    ready,
                    owner,
                })
            }
        }
        pub fn owns_database(&self) -> bool {
            self.owner
        }
        pub fn mark_ready(&self) -> Result<(), McpError> {
            if !self.owner {
                return Err(McpError::new("host_unavailable"));
            }
            if unsafe { windows_sys::Win32::System::Threading::SetEvent(self.ready) } == 0 {
                return Err(unavailable());
            }
            Ok(())
        }
    }
    impl Drop for HostGate {
        fn drop(&mut self) {
            unsafe {
                if self.owner {
                    windows_sys::Win32::System::Threading::ReleaseMutex(self.mutex);
                }
                CloseHandle(self.mutex);
                CloseHandle(self.ready);
            }
        }
    }
    pub fn seal(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>, McpError> {
        let blob = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: null_mut(),
        };
        let success = unsafe {
            if decrypt {
                CryptUnprotectData(
                    &blob,
                    null_mut(),
                    null(),
                    null(),
                    null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            } else {
                CryptProtectData(
                    &blob,
                    null(),
                    null(),
                    null(),
                    null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            }
        };
        if success == 0 {
            return Err(unavailable());
        }
        let result =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
        unsafe {
            LocalFree(out.pbData.cast());
        }
        Ok(result)
    }
    pub fn atomic_replace(from: &Path, to: &Path) -> Result<(), McpError> {
        let a = wide(from.as_os_str());
        let b = wide(to.as_os_str());
        if unsafe {
            MoveFileExW(
                a.as_ptr(),
                b.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(unavailable());
        }
        Ok(())
    }
}

pub fn check_path(path: &Path) -> Result<(), McpError> {
    // Reject reparse points along the whole existing path, not just its leaf.
    for ancestor in path.ancestors() {
        if let Ok(metadata) = std::fs::symlink_metadata(ancestor) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(McpError::new("security_unavailable"));
                }
            }
            if metadata.file_type().is_symlink() {
                return Err(McpError::new("security_unavailable"));
            }
        }
    }
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn startup_gate_waits_for_ready_and_secondary_cannot_own_database() {
        let name = random_secret().unwrap();
        let primary = HostGate::enter(&name).unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let secondary = HostGate::enter(&name).unwrap();
            send.send(secondary.mark_ready().is_err()).unwrap();
        });
        assert!(receive
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err());
        primary.mark_ready().unwrap();
        assert!(receive
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap());
        thread.join().unwrap();
    }
    #[test]
    fn directory_acl_is_private_and_weakened_acl_is_rejected() {
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::{
                Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
            },
        };
        let root = std::env::temp_dir().join(format!("inline-acl-{}", random_secret().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        let descriptor = Descriptor::new(true).unwrap();
        descriptor.apply(&root).unwrap();
        descriptor.verify(&root).unwrap();
        let value = wide(std::ffi::OsStr::new("D:P(A;OICI;FA;;;WD)"));
        let mut sd = std::ptr::null_mut();
        unsafe {
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    value.as_ptr(),
                    1,
                    &mut sd,
                    std::ptr::null_mut()
                ),
                0
            );
            let path = wide(root.as_os_str());
            assert_ne!(
                SetFileSecurityW(
                    path.as_ptr(),
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    sd
                ),
                0
            );
            LocalFree(sd);
        }
        assert!(descriptor.verify(&root).is_err());
        descriptor.apply(&root).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
