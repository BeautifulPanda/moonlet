// Bluetooth radio (Windows.Devices.Radios). All WinRT calls run on a background
// MTA thread, like the media sessions: waiting for an async result on the STA
// window thread would deadlock.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Foundation::TypedEventHandler;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::Result;

pub const WM_BLUETOOTH: u32 = WM_APP + 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Unknown,
    Missing,
    Off,
    On,
}

impl State {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => State::Missing,
            2 => State::Off,
            3 => State::On,
            _ => State::Unknown,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            State::Unknown => 0,
            State::Missing => 1,
            State::Off => 2,
            State::On => 3,
        }
    }
}

enum Job {
    Refresh,
    Toggle,
    Stop,
}

pub struct Bluetooth {
    jobs: Sender<Job>,
    state: Arc<AtomicU8>,
}

impl Bluetooth {
    pub fn start(hwnd: HWND) -> Self {
        let (jobs, inbox) = channel();
        let state = Arc::new(AtomicU8::new(State::Unknown.to_u8()));
        let worker = Worker {
            hwnd: hwnd.0 as isize,
            jobs: jobs.clone(),
            state: state.clone(),
            radio: None,
        };
        std::thread::spawn(move || worker.run(inbox));
        Self { jobs, state }
    }

    pub fn state(&self) -> State {
        State::from_u8(self.state.load(Ordering::Relaxed))
    }

    pub fn toggle(&self) {
        let _ = self.jobs.send(Job::Toggle);
    }

    pub fn refresh(&self) {
        if self.state() == State::Missing {
            let _ = self.jobs.send(Job::Refresh);
        }
    }
}

impl Drop for Bluetooth {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
    }
}

struct Worker {
    hwnd: isize,
    jobs: Sender<Job>,
    state: Arc<AtomicU8>,
    radio: Option<(Radio, i64)>,
}

impl Worker {
    fn run(mut self, inbox: Receiver<Job>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        self.find();
        while let Ok(job) = inbox.recv() {
            match job {
                Job::Refresh => {
                    if self.radio.is_none() {
                        self.find();
                    } else {
                        self.publish();
                    }
                }
                Job::Toggle => self.toggle(),
                Job::Stop => break,
            }
        }
        if let Some((radio, token)) = self.radio.take() {
            let _ = radio.RemoveStateChanged(token);
        }
    }

    fn find(&mut self) {
        let radio = Radio::GetRadiosAsync()
            .and_then(|op| op.join())
            .ok()
            .and_then(|radios| {
                radios
                    .into_iter()
                    .find(|r| r.Kind().is_ok_and(|k| k == RadioKind::Bluetooth))
            });
        if let Some(radio) = radio {
            let jobs = self.jobs.clone();
            let handler = TypedEventHandler::new(move |_, _| {
                let _ = jobs.send(Job::Refresh);
                Ok(())
            });
            let token = radio.StateChanged(&handler).unwrap_or_default();
            self.radio = Some((radio, token));
        }
        self.publish();
    }

    fn toggle(&mut self) {
        let Some((radio, _)) = &self.radio else {
            return;
        };
        let on = radio.State().is_ok_and(|s| s == RadioState::On);
        let target = if on { RadioState::Off } else { RadioState::On };
        let allowed = Radio::RequestAccessAsync()
            .and_then(|op| op.join())
            .is_ok_and(|status| status == RadioAccessStatus::Allowed);
        let done =
            allowed && set_state(radio, target).is_ok_and(|s| s == RadioAccessStatus::Allowed);
        if !done {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.hwnd as *mut _)),
                    WM_BLUETOOTH,
                    WPARAM(1),
                    LPARAM(0),
                );
            }
        }
        self.publish();
    }

    fn publish(&self) {
        let state = match &self.radio {
            None => State::Missing,
            Some((radio, _)) => match radio.State() {
                Ok(RadioState::On) => State::On,
                Ok(RadioState::Off) => State::Off,
                _ => State::Missing,
            },
        };
        let old = self.state.swap(state.to_u8(), Ordering::Relaxed);
        if old != state.to_u8() {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.hwnd as *mut _)),
                    WM_BLUETOOTH,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
}

fn set_state(radio: &Radio, state: RadioState) -> Result<RadioAccessStatus> {
    radio.SetStateAsync(state)?.join()
}
