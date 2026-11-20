use std::path::Path;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FindCloseChangeNotification,
    FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows::core::HSTRING;

pub struct DirWatch(HANDLE);

impl DirWatch {
    pub fn new(dir: &Path) -> Option<Self> {
        let dir = HSTRING::from(dir.as_os_str());
        unsafe {
            FindFirstChangeNotificationW(
                &dir,
                false,
                FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
            )
            .ok()
            .map(Self)
        }
    }

    pub fn handle(&self) -> HANDLE {
        self.0
    }

    pub fn rearm(&self) {
        unsafe {
            let _ = FindNextChangeNotification(self.0);
        }
    }
}

impl Drop for DirWatch {
    fn drop(&mut self) {
        unsafe {
            let _ = FindCloseChangeNotification(self.0);
        }
    }
}
