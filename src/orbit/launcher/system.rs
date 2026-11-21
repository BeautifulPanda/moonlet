use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LUID, WPARAM};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Power::{
    GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES, SetSuspendState,
};
use windows::Win32::System::Shutdown::{
    EWX_HYBRID_SHUTDOWN, EWX_POWEROFF, EWX_REBOOT, ExitWindowsEx, LockWorkStation,
    SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW,
};
use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, SC_MONITORPOWER, WM_SYSCOMMAND};
use windows::core::PCWSTR;

pub fn sleep(hwnd: HWND) {
    unsafe {
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        let modern = GetPwrCapabilities(&mut caps) && caps.AoAc;
        if modern {
            DefWindowProcW(
                hwnd,
                WM_SYSCOMMAND,
                WPARAM(SC_MONITORPOWER as usize),
                LPARAM(2),
            );
        } else {
            let _ = SetSuspendState(false, false, false);
        }
    }
}

pub fn lock() {
    unsafe {
        let _ = LockWorkStation();
    }
}

pub fn shutdown(restart: bool) {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token).is_ok() {
            let mut luid = LUID::default();
            if LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid).is_ok() {
                let privileges = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                let _ = AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None);
            }
            let _ = CloseHandle(token);
        }
        let flags = if restart {
            EWX_REBOOT
        } else {
            EWX_POWEROFF | EWX_HYBRID_SHUTDOWN
        };
        let _ = ExitWindowsEx(flags, SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED);
    }
}

pub fn bin_info() -> (u64, u64) {
    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if SHQueryRecycleBinW(PCWSTR::null(), &mut info).is_err() {
            return (0, 0);
        }
    }
    (info.i64NumItems.max(0) as u64, info.i64Size.max(0) as u64)
}

pub fn empty_bin() {
    std::thread::spawn(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let _ = SHEmptyRecycleBinW(
            None,
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI,
        );
        CoUninitialize();
    });
}
