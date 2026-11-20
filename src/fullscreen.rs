// Full-screen detection. The shell sends ABN_FULLSCREENAPP to registered
// appbars; this one is registered without reserving any screen space.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Shell::{
    ABM_NEW, ABM_REMOVE, ABN_FULLSCREENAPP, APPBARDATA, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN, SHAppBarMessage, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, WM_APP};

pub const WM_APPBAR: u32 = WM_APP + 2;

pub struct FullscreenWatch {
    hwnd: HWND,
}

impl FullscreenWatch {
    pub fn register(hwnd: HWND) -> Self {
        let watch = Self { hwnd };
        watch.renew();
        watch
    }

    pub fn renew(&self) {
        let mut data = Self::data(self.hwnd);
        data.uCallbackMessage = WM_APPBAR;
        unsafe {
            SHAppBarMessage(ABM_NEW, &mut data);
        }
    }

    fn data(hwnd: HWND) -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..Default::default()
        }
    }
}

impl Drop for FullscreenWatch {
    fn drop(&mut self) {
        let mut data = Self::data(self.hwnd);
        unsafe {
            SHAppBarMessage(ABM_REMOVE, &mut data);
        }
    }
}

pub fn active_now() -> bool {
    let state = unsafe { SHQueryUserNotificationState() };
    matches!(
        state,
        Ok(QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
    )
}

pub fn parse(wparam: WPARAM, lparam: LPARAM) -> Option<bool> {
    if wparam.0 as u32 != ABN_FULLSCREENAPP {
        return None;
    }
    let entered = lparam.0 != 0;
    // Show desktop (Win+D) is sometimes reported as full screen.
    if entered && foreground_is_desktop() {
        return None;
    }
    Some(entered)
}

fn foreground_is_desktop() -> bool {
    let mut name = [0u16; 32];
    let len = unsafe { GetClassNameW(GetForegroundWindow(), &mut name) } as usize;
    let class = String::from_utf16_lossy(&name[..len]);
    class == "WorkerW" || class == "Progman"
}
