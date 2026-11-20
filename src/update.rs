// Self-update from GitHub releases. A release carries moonlet.exe and
// update.txt with "version", "sha256" and "signature" lines; the signature is
// ECDSA P-256 over "moonlet <version> <sha256>" and is checked with the
// built-in public key, so a compromised release page cannot ship a file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::Win32::Security::Cryptography::{
    BCRYPT_ALG_HANDLE, BCRYPT_ECCPUBLIC_BLOB, BCRYPT_ECDSA_P256_ALGORITHM, BCRYPT_FLAGS,
    BCRYPT_KEY_HANDLE, BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS, BCRYPT_SHA256_ALG_HANDLE,
    BCryptCloseAlgorithmProvider, BCryptDestroyKey, BCryptHash, BCryptImportKeyPair,
    BCryptOpenAlgorithmProvider, BCryptVerifySignature,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SW_SHOWNORMAL, WM_APP};
use windows::core::{HSTRING, PCWSTR, w};

use crate::lang::tr;

pub const WM_UPDATE: u32 = WM_APP + 5;

const RELEASES: &str = "https://github.com/BeautifulPanda/moonlet/releases";

const PUBLIC_KEY: [u8; 72] = [
    0x45, 0x43, 0x53, 0x31, 0x20, 0x00, 0x00, 0x00, 0x82, 0x90, 0xE6, 0x8A, 0x42, 0xCC, 0x56, 0x11,
    0xBF, 0xF6, 0x49, 0xC4, 0x26, 0x08, 0xA5, 0x4B, 0xA4, 0x6C, 0x2C, 0xCA, 0x05, 0xC0, 0x1C, 0x25,
    0x24, 0x4B, 0x15, 0x20, 0x8D, 0x6F, 0x99, 0xE6, 0xA1, 0x63, 0xAA, 0x16, 0x24, 0x9D, 0xFA, 0x11,
    0x2E, 0x97, 0xB9, 0x35, 0x19, 0x31, 0x68, 0x27, 0x90, 0xAA, 0x4A, 0xB1, 0x00, 0x11, 0x04, 0xB7,
    0xFA, 0xB8, 0xA6, 0x11, 0x0E, 0x5C, 0x33, 0xA8,
];

const MANIFEST_LIMIT: usize = 4 * 1024;
const EXE_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Auto,
    Check,
    Off,
}

impl Mode {
    pub const ALL: [(Mode, &'static str); 3] = [
        (Mode::Auto, "auto"),
        (Mode::Check, "check"),
        (Mode::Off, "off"),
    ];

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(mode, _)| *mode == self)
            .map_or("auto", |(_, name)| name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(mode, _)| *mode)
    }
}

pub enum Outcome {
    UpToDate,
    Available(String),
    Ready { staged: PathBuf },
    Failed(&'static str),
}

pub struct Updater {
    hwnd: HWND,
    result: Arc<Mutex<Option<Outcome>>>,
    busy: Arc<AtomicBool>,
}

impl Updater {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            result: Arc::default(),
            busy: Arc::default(),
        }
    }

    pub fn start(&self, download: bool) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let (result, busy) = (self.result.clone(), self.busy.clone());
        // HWND is not Send; pass the raw value.
        let hwnd = self.hwnd.0 as isize;
        std::thread::spawn(move || {
            let outcome = check(download);
            if let Ok(mut slot) = result.lock() {
                *slot = Some(outcome);
            }
            busy.store(false, Ordering::Release);
            unsafe {
                let hwnd = HWND(hwnd as *mut _);
                let _ = PostMessageW(Some(hwnd), WM_UPDATE, WPARAM(0), LPARAM(0));
            }
        });
    }

    pub fn take(&self) -> Option<Outcome> {
        self.result.lock().ok()?.take()
    }
}

pub fn current_version() -> String {
    #[cfg(feature = "dev")]
    if let Some(version) = std::env::var_os("MOONLET_FAKE_VERSION") {
        return version.to_string_lossy().into_owned();
    }
    env!("CARGO_PKG_VERSION").to_string()
}

fn releases() -> String {
    #[cfg(feature = "dev")]
    if let Some(url) = std::env::var_os("MOONLET_UPDATE_URL") {
        return url.to_string_lossy().into_owned();
    }
    RELEASES.to_string()
}

fn check(download: bool) -> Outcome {
    let failed = || {
        Outcome::Failed(tr(
            "не удалось связаться с GitHub",
            "could not reach GitHub",
        ))
    };
    let base = releases();
    let Some(manifest) = http_get(
        &format!("{base}/latest/download/update.txt"),
        MANIFEST_LIMIT,
    ) else {
        return failed();
    };
    let Some((version, sha256)) = String::from_utf8(manifest)
        .ok()
        .and_then(|text| verified_manifest(&text))
    else {
        return Outcome::Failed(tr(
            "описание выпуска не прошло проверку подписи",
            "the release description failed the signature check",
        ));
    };
    if !is_newer(&version, &current_version()) {
        return Outcome::UpToDate;
    }
    if !download {
        return Outcome::Available(version);
    }
    let Some(exe) = http_get(
        &format!("{base}/download/v{version}/moonlet.exe"),
        EXE_LIMIT,
    ) else {
        return failed();
    };
    if sha256_of(&exe) != Some(sha256) {
        return Outcome::Failed(tr(
            "скачанный файл не совпал с подписанным",
            "the downloaded file does not match the signed one",
        ));
    }
    let Some(staged) = staged_path() else {
        return Outcome::Failed(tr("не найден файл программы", "program file not found"));
    };
    if std::fs::write(&staged, &exe).is_err() {
        return Outcome::Failed(tr(
            "нет прав на запись в папку программы — скачайте новую версию вручную",
            "no permission to write to the program folder; download the new version manually",
        ));
    }
    Outcome::Ready { staged }
}

fn verified_manifest(text: &str) -> Option<(String, [u8; 32])> {
    let (version, sha256_hex, signature_hex) = parse_manifest(text)?;
    let sha256: [u8; 32] = from_hex(&sha256_hex)?.try_into().ok()?;
    let signature = from_hex(&signature_hex)?;
    let message = format!("moonlet {version} {}", sha256_hex.to_ascii_lowercase());
    let digest = sha256_of(message.as_bytes())?;
    verify(&digest, &signature).then_some((version, sha256))
}

fn parse_manifest(text: &str) -> Option<(String, String, String)> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == name).then(|| value.trim().to_string())
        })
    };
    Some((field("version")?, field("sha256")?, field("signature")?))
}

fn is_newer(a: &str, b: &str) -> bool {
    match (parse_version(a), parse_version(b)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn parse_version(text: &str) -> Option<[u32; 3]> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut version = [0; 3];
    for slot in &mut version {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(version)
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn sha256_of(data: &[u8]) -> Option<[u8; 32]> {
    let mut digest = [0u8; 32];
    unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut digest) }
        .is_ok()
        .then_some(digest)
}

fn verify(digest: &[u8; 32], signature: &[u8]) -> bool {
    unsafe {
        let mut algorithm = BCRYPT_ALG_HANDLE::default();
        if BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_ECDSA_P256_ALGORITHM,
            PCWSTR::null(),
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .is_err()
        {
            return false;
        }
        let mut key = BCRYPT_KEY_HANDLE::default();
        let mut valid = false;
        if BCryptImportKeyPair(
            algorithm,
            None,
            BCRYPT_ECCPUBLIC_BLOB,
            &mut key,
            &PUBLIC_KEY,
            0,
        )
        .is_ok()
        {
            valid = BCryptVerifySignature(key, None, digest, signature, BCRYPT_FLAGS(0)).is_ok();
            let _ = BCryptDestroyKey(key);
        }
        let _ = BCryptCloseAlgorithmProvider(algorithm, 0);
        valid
    }
}

fn http_get(url: &str, limit: usize) -> Option<Vec<u8>> {
    let (secure, host, port, path) = split_url(url)?;
    let agent = HSTRING::from(format!("Moonlet/{}", env!("CARGO_PKG_VERSION")));
    unsafe {
        let session = WinHttpOpen(
            &agent,
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return None;
        }
        let _ = WinHttpSetTimeouts(session, 15_000, 15_000, 15_000, 30_000);
        let connection = WinHttpConnect(session, &HSTRING::from(host), port, 0);
        let mut body = None;
        if !connection.is_null() {
            let flags = if secure {
                WINHTTP_FLAG_SECURE
            } else {
                WINHTTP_OPEN_REQUEST_FLAGS(0)
            };
            let request = WinHttpOpenRequest(
                connection,
                w!("GET"),
                &HSTRING::from(path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                flags,
            );
            if !request.is_null() {
                body = read_response(request, limit);
                let _ = WinHttpCloseHandle(request);
            }
            let _ = WinHttpCloseHandle(connection);
        }
        let _ = WinHttpCloseHandle(session);
        body
    }
}

unsafe fn read_response(request: *mut core::ffi::c_void, limit: usize) -> Option<Vec<u8>> {
    unsafe {
        WinHttpSendRequest(request, None, None, 0, 0, 0).ok()?;
        WinHttpReceiveResponse(request, std::ptr::null_mut()).ok()?;
        let mut status = 0u32;
        let mut size = size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut _),
            &mut size,
            std::ptr::null_mut(),
        )
        .ok()?;
        if status != 200 {
            return None;
        }
        let mut body = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(
                request,
                chunk.as_mut_ptr() as *mut _,
                chunk.len() as u32,
                &mut read,
            )
            .ok()?;
            if read == 0 {
                return Some(body);
            }
            body.extend_from_slice(&chunk[..read as usize]);
            if body.len() > limit {
                return None;
            }
        }
    }
}

// "https://host:port/path" → (https?, host, port, path).
fn split_url(url: &str) -> Option<(bool, String, u16, String)> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else {
        (false, url.strip_prefix("http://")?)
    };
    let (authority, path) = match rest.find('/') {
        Some(slash) => (&rest[..slash], &rest[slash..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    (!host.is_empty()).then(|| (secure, host.to_string(), port, path.to_string()))
}

fn staged_path() -> Option<PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .with_file_name("moonlet.new.exe"),
    )
}

fn old_path(exe: &Path) -> PathBuf {
    exe.with_file_name("moonlet.old.exe")
}

// A running exe can be renamed but not overwritten: move it aside, put the
// new one in its place and start it.
pub fn install(staged: &Path) -> Result<(), &'static str> {
    let no_access = tr(
        "не удалось заменить файл программы — скачайте новую версию вручную",
        "could not replace the program file; download the new version manually",
    );
    let exe = std::env::current_exe().map_err(|_| no_access)?;
    let old = old_path(&exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&exe, &old).map_err(|_| no_access)?;
    if std::fs::rename(staged, &exe).is_err() {
        let _ = std::fs::rename(&old, &exe);
        return Err(no_access);
    }
    let started = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(exe.as_os_str()),
            w!("--updated"),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if started.0 as usize <= 32 {
        let _ = std::fs::rename(&exe, staged);
        let _ = std::fs::rename(&old, &exe);
        return Err(tr(
            "новая версия не запустилась, оставлена прежняя",
            "the new version did not start; the old one is kept",
        ));
    }
    Ok(())
}

pub fn clean_up() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
        if let Some(staged) = staged_path() {
            let _ = std::fs::remove_file(staged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.9.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("0.1", "0.0.1"));
        assert!(!is_newer("abc", "0.0.1"));
    }

    #[test]
    fn splits_urls() {
        assert_eq!(
            split_url("https://github.com/a/b/releases/latest"),
            Some((
                true,
                "github.com".into(),
                443,
                "/a/b/releases/latest".into()
            ))
        );
        assert_eq!(
            split_url("http://localhost:8765/x"),
            Some((false, "localhost".into(), 8765, "/x".into()))
        );
        assert_eq!(split_url("ftp://x"), None);
    }

    #[test]
    fn reads_manifest_and_hex() {
        let text = "version = 0.1.1\r\nsha256 = AB01\nsignature=ff\n";
        assert_eq!(
            parse_manifest(text),
            Some(("0.1.1".into(), "AB01".into(), "ff".into()))
        );
        assert_eq!(from_hex("AB01"), Some(vec![0xAB, 0x01]));
        assert_eq!(from_hex("ABC"), None);
        assert_eq!(from_hex("zz"), None);
    }

    #[test]
    fn rejects_forged_signature() {
        let digest = sha256_of(b"moonlet 9.9.9 00").unwrap();
        assert!(!verify(&digest, &[0u8; 64]));
    }
}
