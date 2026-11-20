// Media sessions (GSMTC). All WinRT calls run on a background MTA thread:
// blocking on an async operation on the STA window thread deadlocks,
// because the completion is delivered through that same message queue.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{Result, RuntimeType};

use super::motion::now;

pub const WM_MEDIA: u32 = WM_APP + 3;

const MAX_COVER_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub sampled_at: f64,
}

impl Track {
    pub fn progress(&self, t: f64) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        let position = if self.playing {
            self.position + (t - self.sampled_at)
        } else {
            self.position
        };
        (position / self.duration).clamp(0.0, 1.0) as f32
    }
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub track: Option<Track>,
    pub cover: Option<Arc<[u8]>>,
    pub cover_id: u64,
    pub app: String,
}

#[derive(Clone, Copy)]
enum Job {
    Refresh,
    Properties,
    Toggle,
    Next,
    Previous,
    Stop,
}

pub struct Media {
    jobs: Sender<Job>,
    snapshot: Arc<Mutex<Snapshot>>,
}

impl Media {
    pub fn start(hwnd: HWND) -> Self {
        let (jobs, inbox) = channel();
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let worker = Worker {
            hwnd: hwnd.0 as isize,
            jobs: jobs.clone(),
            snapshot: snapshot.clone(),
            manager: None,
            session: None,
            track_key: None,
            cover: None,
        };
        std::thread::spawn(move || worker.run(inbox));
        Self { jobs, snapshot }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn toggle(&self) {
        let _ = self.jobs.send(Job::Toggle);
    }

    pub fn next(&self) {
        let _ = self.jobs.send(Job::Next);
    }

    pub fn previous(&self) {
        let _ = self.jobs.send(Job::Previous);
    }
}

impl Drop for Media {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
    }
}

struct Worker {
    hwnd: isize,
    jobs: Sender<Job>,
    snapshot: Arc<Mutex<Snapshot>>,
    manager: Option<(Manager, [i64; 2])>,
    session: Option<(Session, [i64; 3])>,
    track_key: Option<(String, String)>,
    cover: Option<Arc<[u8]>>,
}

impl Worker {
    fn run(mut self, inbox: Receiver<Job>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        if self.connect().is_ok() {
            self.refresh(true);
        }
        while let Ok(first) = inbox.recv() {
            let (mut refresh, mut properties) = (false, false);
            for job in std::iter::once(first).chain(inbox.try_iter()) {
                match job {
                    Job::Refresh => refresh = true,
                    Job::Properties => properties = true,
                    Job::Toggle => self.with_session(|s| s.TryTogglePlayPauseAsync().map(drop)),
                    Job::Next => self.with_session(|s| s.TrySkipNextAsync().map(drop)),
                    Job::Previous => self.with_session(|s| s.TrySkipPreviousAsync().map(drop)),
                    Job::Stop => {
                        self.disconnect();
                        return;
                    }
                }
            }
            if refresh || properties {
                self.refresh(properties);
            }
        }
        self.disconnect();
    }

    fn connect(&mut self) -> Result<()> {
        let manager = Manager::RequestAsync()?.join()?;
        let tokens = [
            manager.CurrentSessionChanged(&self.handler(Job::Refresh))?,
            manager
                .SessionsChanged(&self.handler(Job::Refresh))
                .unwrap_or_default(),
        ];
        self.manager = Some((manager, tokens));
        Ok(())
    }

    fn disconnect(&mut self) {
        self.unsubscribe();
        if let Some((manager, [current, sessions])) = self.manager.take() {
            let _ = manager.RemoveCurrentSessionChanged(current);
            let _ = manager.RemoveSessionsChanged(sessions);
        }
    }

    // Stick to the shown session while it plays, so two sources playing at
    // once do not make the satellite flip between them.
    fn pick(&self, manager: &Manager) -> Option<Session> {
        let current = manager.GetCurrentSession().ok();
        let sessions: Vec<Session> = manager
            .GetSessions()
            .map(|list| list.into_iter().collect())
            .unwrap_or_default();
        let playing = |session: &Session| {
            session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .is_ok_and(|status| status == Status::Playing)
        };
        let shown = self
            .session
            .as_ref()
            .and_then(|(shown, _)| sessions.iter().find(|s| *s == shown));
        if let Some(shown) = shown
            && playing(shown)
        {
            return Some(shown.clone());
        }
        if let Some(current) = &current
            && playing(current)
        {
            return Some(current.clone());
        }
        if let Some(any) = sessions.iter().find(|s| playing(s)) {
            return Some(any.clone());
        }
        shown.cloned().or(current)
    }

    fn with_session(&self, action: impl FnOnce(&Session) -> Result<()>) {
        if let Some((session, _)) = &self.session {
            let _ = action(session);
        }
    }

    fn refresh(&mut self, properties: bool) {
        let Some((manager, _)) = &self.manager else {
            return;
        };
        let chosen = self.pick(manager);
        let switched = match (&self.session, &chosen) {
            (Some((old, _)), Some(new)) => old != new,
            (None, None) => false,
            _ => true,
        };
        if switched {
            self.unsubscribe();
            if let Some(session) = &chosen {
                self.subscribe(session);
            }
        }

        let (track, thumbnail) = match chosen.as_ref().and_then(|s| read_track(s).ok()) {
            Some((track, thumbnail)) => (Some(track), thumbnail),
            None => (None, None),
        };
        let app = chosen
            .as_ref()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|id| id.to_string())
            .unwrap_or_default();
        let key = track.as_ref().map(|t| (t.title.clone(), t.artist.clone()));
        let reread = properties || switched || key != self.track_key;
        self.track_key = key;
        let mut cover_changed = false;
        if reread {
            let cover: Option<Arc<[u8]>> =
                thumbnail.and_then(|t| read_cover(&t).ok()).map(Arc::from);
            if cover.as_deref() != self.cover.as_deref() {
                self.cover = cover;
                cover_changed = true;
            }
        }

        if let Ok(mut snapshot) = self.snapshot.lock() {
            snapshot.track = track;
            snapshot.app = app;
            if cover_changed {
                snapshot.cover = self.cover.clone();
                snapshot.cover_id += 1;
            }
        }
        unsafe {
            let hwnd = HWND(self.hwnd as *mut _);
            let _ = PostMessageW(Some(hwnd), WM_MEDIA, WPARAM(0), LPARAM(0));
        }
    }

    fn subscribe(&mut self, session: &Session) {
        let tokens = [
            session
                .MediaPropertiesChanged(&self.handler(Job::Properties))
                .unwrap_or_default(),
            session
                .PlaybackInfoChanged(&self.handler(Job::Refresh))
                .unwrap_or_default(),
            session
                .TimelinePropertiesChanged(&self.handler(Job::Refresh))
                .unwrap_or_default(),
        ];
        self.session = Some((session.clone(), tokens));
    }

    fn unsubscribe(&mut self) {
        if let Some((session, [props, playback, timeline])) = self.session.take() {
            let _ = session.RemoveMediaPropertiesChanged(props);
            let _ = session.RemovePlaybackInfoChanged(playback);
            let _ = session.RemoveTimelinePropertiesChanged(timeline);
        }
    }

    fn handler<S: RuntimeType + 'static, A: RuntimeType + 'static>(
        &self,
        job: Job,
    ) -> TypedEventHandler<S, A> {
        let jobs = self.jobs.clone();
        TypedEventHandler::new(move |_, _| {
            let _ = jobs.send(job);
            Ok(())
        })
    }
}

fn read_track(session: &Session) -> Result<(Track, Option<IRandomAccessStreamReference>)> {
    let props = session.TryGetMediaPropertiesAsync()?.join()?;
    let info = session.GetPlaybackInfo()?;
    let timeline = session.GetTimelineProperties()?;

    let playing = info.PlaybackStatus()? == Status::Playing;
    let ticks = |span: windows::Foundation::TimeSpan| span.Duration as f64 / 1e7;
    let duration = ticks(timeline.EndTime()?) - ticks(timeline.StartTime()?);
    let mut position = ticks(timeline.Position()?);
    let updated = timeline.LastUpdatedTime()?.UniversalTime;
    if playing && updated > 0 {
        let elapsed = (filetime_now() - updated) as f64 / 1e7;
        if (0.0..duration.max(0.0)).contains(&elapsed) {
            position += elapsed;
        }
    }

    let track = Track {
        title: props.Title()?.to_string(),
        artist: props.Artist()?.to_string(),
        playing,
        position: position.clamp(0.0, duration.max(0.0)),
        duration,
        sampled_at: now(),
    };
    Ok((track, props.Thumbnail().ok()))
}

fn read_cover(thumbnail: &IRandomAccessStreamReference) -> Result<Vec<u8>> {
    let stream = thumbnail.OpenReadAsync()?.join()?;
    let size = stream.Size()?.min(MAX_COVER_BYTES) as u32;
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

fn filetime_now() -> i64 {
    let time = unsafe { GetSystemTimePreciseAsFileTime() };
    ((time.dwHighDateTime as i64) << 32) | time.dwLowDateTime as i64
}
