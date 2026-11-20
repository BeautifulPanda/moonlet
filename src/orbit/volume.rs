// Volume of the app that plays the media session, as in the Windows volume
// mixer; falls back to the master volume when the app is not found there.

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::{Interface, PWSTR};

pub struct Volume {
    devices: IMMDeviceEnumerator,
}

enum Target {
    App(Vec<ISimpleAudioVolume>),
    System(IAudioEndpointVolume),
}

#[derive(Clone, Copy)]
pub struct Level {
    pub level: f32,
    pub muted: bool,
    pub system: bool,
}

impl Volume {
    pub fn new() -> Option<Self> {
        let devices = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.ok()?;
        Some(Self { devices })
    }

    fn target(&self, app: &str) -> Option<Target> {
        unsafe {
            let device = self
                .devices
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .ok()?;
            let sessions = app_sessions(&device, app);
            if !sessions.is_empty() {
                return Some(Target::App(sessions));
            }
            device.Activate(CLSCTX_ALL, None).ok().map(Target::System)
        }
    }

    pub fn get(&self, app: &str) -> Option<Level> {
        unsafe {
            match self.target(app)? {
                Target::App(sessions) => {
                    let first = sessions.first()?;
                    Some(Level {
                        level: first.GetMasterVolume().ok()?,
                        muted: first.GetMute().ok()?.as_bool(),
                        system: false,
                    })
                }
                Target::System(endpoint) => Some(Level {
                    level: endpoint.GetMasterVolumeLevelScalar().ok()?,
                    muted: endpoint.GetMute().ok()?.as_bool(),
                    system: true,
                }),
            }
        }
    }

    pub fn set(&self, app: &str, level: f32) {
        let level = level.clamp(0.0, 1.0);
        unsafe {
            match self.target(app) {
                Some(Target::App(sessions)) => {
                    for session in sessions {
                        let _ = session.SetMasterVolume(level, std::ptr::null());
                        let _ = session.SetMute(false, std::ptr::null());
                    }
                }
                Some(Target::System(endpoint)) => {
                    let _ = endpoint.SetMasterVolumeLevelScalar(level, std::ptr::null());
                    let _ = endpoint.SetMute(false, std::ptr::null());
                }
                None => {}
            }
        }
    }

    pub fn change(&self, app: &str, delta: f32) {
        if let Some(current) = self.get(app) {
            self.set(app, current.level + delta);
        }
    }

    pub fn toggle_mute(&self, app: &str) {
        let Some(current) = self.get(app) else {
            return;
        };
        let mute = !current.muted;
        unsafe {
            match self.target(app) {
                Some(Target::App(sessions)) => {
                    for session in sessions {
                        let _ = session.SetMute(mute, std::ptr::null());
                    }
                }
                Some(Target::System(endpoint)) => {
                    let _ = endpoint.SetMute(mute, std::ptr::null());
                }
                None => {}
            }
        }
    }
}

fn app_sessions(device: &IMMDevice, app: &str) -> Vec<ISimpleAudioVolume> {
    let key = app_key(app);
    let mut found = Vec::new();
    if key.is_empty() {
        return found;
    }
    unsafe {
        let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else {
            return found;
        };
        let Ok(list) = manager.GetSessionEnumerator() else {
            return found;
        };
        for index in 0..list.GetCount().unwrap_or(0) {
            let Ok(control) = list.GetSession(index) else {
                continue;
            };
            let Ok(pid) = control
                .cast::<IAudioSessionControl2>()
                .and_then(|c| c.GetProcessId())
            else {
                continue;
            };
            if pid != 0
                && process_name(pid).is_some_and(|name| matches_app(&key, &name))
                && let Ok(volume) = control.cast::<ISimpleAudioVolume>()
            {
                found.push(volume);
            }
        }
    }
    found
}

fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(process);
        ok.then(|| file_key(&String::from_utf16_lossy(&buffer[..size as usize])))
    }
}

fn app_key(app: &str) -> String {
    file_key(app)
}

fn file_key(path: &str) -> String {
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase();
    name.strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(name)
}

fn matches_app(key: &str, process: &str) -> bool {
    key == process || (process.len() >= 4 && key.contains(process))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_matches(app: &str, exe: &str) -> bool {
        matches_app(&app_key(app), &file_key(exe))
    }

    #[test]
    fn finds_media_apps_in_the_mixer() {
        assert!(app_matches(
            "Spotify.exe",
            r"C:\Users\x\AppData\Roaming\Spotify\Spotify.exe"
        ));
        assert!(app_matches(
            "Chrome",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe"
        ));
        assert!(app_matches(
            "MSEdge",
            r"C:\Program Files (x86)\Microsoft\Edge\msedge.exe"
        ));
        assert!(app_matches(
            "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
            r"C:\Program Files\WindowsApps\Spotify.exe"
        ));
        assert!(!app_matches("Chrome", r"C:\Windows\explorer.exe"));
        assert!(!app_matches("", r"C:\Windows\explorer.exe"));
    }
}
