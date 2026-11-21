use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use windows::Storage::Search::{FolderDepth, IndexerOption, QueryOptions};
use windows::Storage::{StorageFolder, StorageItemTypes};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    IPersistFile, STGM_READ,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Recent, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, Interface, Result};

use super::search::Found;

pub const WM_FILES: u32 = WM_APP + 6;

const RECENT_LIMIT: usize = 40;
const INDEX_LIMIT: u32 = 60;

#[derive(Default)]
pub struct Results {
    pub recent: Vec<Found>,
    pub found: Vec<Found>,
}

enum Job {
    Search(String),
    Stop,
}

pub struct Files {
    jobs: Sender<Job>,
    pub results: Arc<Mutex<Results>>,
}

impl Files {
    pub fn start(hwnd: HWND) -> Self {
        let (jobs, inbox) = channel();
        let results = Arc::new(Mutex::new(Results::default()));
        let shared = results.clone();
        let hwnd = hwnd.0 as isize;
        std::thread::spawn(move || run(HWND(hwnd as *mut _), inbox, shared));
        Self { jobs, results }
    }

    pub fn search(&self, text: &str) {
        let _ = self.jobs.send(Job::Search(text.to_string()));
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
    }
}

fn run(hwnd: HWND, inbox: Receiver<Job>, results: Arc<Mutex<Results>>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let notify = || unsafe {
        let _ = PostMessageW(Some(hwnd), WM_FILES, WPARAM(0), LPARAM(0));
    };
    let recent = recent_files();
    if let Ok(mut r) = results.lock() {
        r.recent = recent;
    }
    notify();

    let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let mut folder: Option<StorageFolder> = None;
    while let Ok(first) = inbox.recv() {
        let mut text = None;
        for job in std::iter::once(first).chain(inbox.try_iter()) {
            match job {
                Job::Search(t) => text = Some(t),
                Job::Stop => return,
            }
        }
        let Some(text) = text else {
            continue;
        };
        if folder.is_none()
            && let Some(profile) = &profile
        {
            folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(profile.as_os_str()))
                .and_then(|op| op.join())
                .ok();
        }
        let found = match &folder {
            Some(folder) => index_search(folder, &text).unwrap_or_default(),
            None => Vec::new(),
        };
        if let Ok(mut r) = results.lock() {
            r.found = found;
        }
        notify();
    }
}

fn index_search(folder: &StorageFolder, text: &str) -> Result<Vec<Found>> {
    let text = text.replace('"', " ");
    let text = text.trim();
    let mut found = query(folder, &format!("System.FileName:~<\"{text}\""))?;
    if found.len() < ENOUGH {
        for item in query(folder, &format!("System.FileName:~=\"{text}\""))? {
            if !found.iter().any(|f| f.path == item.path) {
                found.push(item);
            }
        }
    }
    Ok(found)
}

const ENOUGH: usize = 8;

fn query(folder: &StorageFolder, filter: &str) -> Result<Vec<Found>> {
    let options = QueryOptions::new()?;
    options.SetFolderDepth(FolderDepth::Deep)?;
    options.SetIndexerOption(IndexerOption::OnlyUseIndexer)?;
    options.SetUserSearchFilter(&HSTRING::from(filter))?;
    let query = folder.CreateItemQueryWithOptions(&options)?;
    let items = query.GetItemsAsync(0, INDEX_LIMIT)?.join()?;
    let mut found = Vec::new();
    for item in items {
        let (Ok(path), Ok(name)) = (item.Path(), item.Name()) else {
            continue;
        };
        let path = PathBuf::from(path.to_os_string());
        if is_hidden_place(&path) {
            continue;
        }
        let folder = item.IsOfType(StorageItemTypes::Folder).unwrap_or(false);
        found.push(Found::new(name.to_string(), path, folder, false));
    }
    Ok(found)
}

fn is_hidden_place(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.components().any(|c| {
            let part = c.as_os_str().to_string_lossy();
            part.starts_with('.')
                || part.starts_with("__")
                || ["AppData", "node_modules", "site-packages"]
                    .iter()
                    .any(|name| part.eq_ignore_ascii_case(name))
        })
    })
}

fn recent_files() -> Vec<Found> {
    let Some(dir) = recent_folder() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut links: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.to_ascii_lowercase().ends_with(".lnk") && !name.starts_with("ms-")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    links.sort_by_key(|link| std::cmp::Reverse(link.0));
    let mut found = Vec::new();
    for (_, link) in links {
        if found.len() >= RECENT_LIMIT {
            break;
        }
        let Some(target) = link_target(&link) else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(&target) else {
            continue;
        };
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| target.to_string_lossy().into_owned());
        found.push(Found::new(name, target, meta.is_dir(), true));
    }
    found
}

fn recent_folder() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Recent, KF_FLAG_DEFAULT, None).ok()?;
        let text = path.to_string().ok();
        CoTaskMemFree(Some(path.0 as *const _));
        text.map(PathBuf::from)
    }
}

fn link_target(link: &Path) -> Option<PathBuf> {
    unsafe {
        let shell_link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = shell_link.cast().ok()?;
        file.Load(&HSTRING::from(link.as_os_str()), STGM_READ)
            .ok()?;
        let mut buffer = [0u16; 260];
        shell_link
            .GetPath(&mut buffer, std::ptr::null_mut(), 0)
            .ok()?;
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        (end > 0).then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..end])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn reads_recent_files_and_the_index() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let started = std::time::Instant::now();
        let recent = recent_files();
        println!("recent: {} in {:?}", recent.len(), started.elapsed());
        for f in recent.iter().take(5) {
            println!("  {f:?}");
        }
        let profile = std::env::var_os("USERPROFILE").unwrap();
        let folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(profile))
            .unwrap()
            .join()
            .unwrap();
        for text in ["moonlet", "cargo", "новый", "main"] {
            let started = std::time::Instant::now();
            let found = index_search(&folder, text).unwrap();
            println!("{text}: {} in {:?}", found.len(), started.elapsed());
            for f in found.iter().take(6) {
                println!("  {} | {}", f.name, f.path.display());
            }
        }
    }
}
