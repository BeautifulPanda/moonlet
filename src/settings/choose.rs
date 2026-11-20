use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_NODEREFERENCELINKS, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::lang::tr;

pub fn path(owner: HWND, folder: bool, programs: bool) -> Option<String> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let mut options = dialog.GetOptions().ok()? | FOS_FORCEFILESYSTEM | FOS_NODEREFERENCELINKS;
        if folder {
            options |= FOS_PICKFOLDERS;
        }
        dialog.SetOptions(options).ok()?;
        let title = HSTRING::from(if folder {
            tr("Папка для плитки хаба", "Folder for a hub tile")
        } else if programs {
            tr("Программа для плитки хаба", "App for a hub tile")
        } else {
            tr("Файл для плитки хаба", "File for a hub tile")
        });
        let _ = dialog.SetTitle(&title);
        let apps_name = HSTRING::from(tr("Программы и ярлыки", "Apps and shortcuts"));
        let all_name = HSTRING::from(tr("Все файлы", "All files"));
        if programs {
            let filters = [
                COMDLG_FILTERSPEC {
                    pszName: PCWSTR(apps_name.as_ptr()),
                    pszSpec: w!("*.exe;*.lnk;*.url;*.bat;*.cmd"),
                },
                COMDLG_FILTERSPEC {
                    pszName: PCWSTR(all_name.as_ptr()),
                    pszSpec: w!("*.*"),
                },
            ];
            let _ = dialog.SetFileTypes(&filters);
        }
        dialog.Show(Some(owner)).ok()?;
        let item = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        path
    }
}
