//! GSMTC lives on one MTA worker. Callbacks enqueue hints, never touch UI state.
use crate::{
    message::Message,
    modules::music::{Capabilities, MediaCommand, MusicInfo, Repeat, Timeline},
    services::media::{MediaError, MediaProvider, MediaRequest, matches_source},
};
use iced::{Subscription, futures::SinkExt};
use std::{
    sync::mpsc::{self, SyncSender},
    time::{Duration, Instant},
};
use windows::{
    Foundation::TypedEventHandler,
    Management::Deployment::PackageManager,
    Media::{
        Control::{
            GlobalSystemMediaTransportControlsSession as Session,
            GlobalSystemMediaTransportControlsSessionManager as Manager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
        },
        MediaPlaybackAutoRepeatMode,
    },
    Storage::Streams::DataReader,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::HSTRING,
};

#[derive(Debug)]
pub enum WorkerRequest {
    Refresh {
        generation: Option<u64>,
        artwork: bool,
    },
    Command(MediaRequest),
}

impl From<windows::core::Error> for MediaError {
    fn from(error: windows::core::Error) -> Self {
        Self::Native(error.to_string())
    }
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, MediaError> {
        // SAFETY: this dedicated thread has no existing apartment and balances initialization.
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

struct BoundSession {
    session: Session,
    media: Option<i64>,
    playback: Option<i64>,
    timeline: Option<i64>,
}
impl Drop for BoundSession {
    fn drop(&mut self) {
        if let Some(token) = self.media {
            let _ = self.session.RemoveMediaPropertiesChanged(token);
        }
        if let Some(token) = self.playback {
            let _ = self.session.RemovePlaybackInfoChanged(token);
        }
        if let Some(token) = self.timeline {
            let _ = self.session.RemoveTimelinePropertiesChanged(token);
        }
    }
}

pub struct WindowsMediaProvider {
    manager: Manager,
    sessions_token: Option<i64>,
    current_token: Option<i64>,
    selected: Option<BoundSession>,
    families: Vec<String>,
    explicit: Option<String>,
    tx: SyncSender<WorkerRequest>,
    generation: u64,
    cache: MusicInfo,
    metadata_key: String,
    timeline_stamp: Option<(i64, i64)>,
}
impl Drop for WindowsMediaProvider {
    fn drop(&mut self) {
        self.selected.take();
        if let Some(token) = self.sessions_token {
            let _ = self.manager.RemoveSessionsChanged(token);
        }
        if let Some(token) = self.current_token {
            let _ = self.manager.RemoveCurrentSessionChanged(token);
        }
    }
}

impl WindowsMediaProvider {
    fn new(tx: SyncSender<WorkerRequest>, explicit: Option<String>) -> Result<Self, MediaError> {
        let manager = Manager::RequestAsync()?.join()?;
        let mut families = Vec::new();
        if explicit.is_none() {
            for package in PackageManager::new()?.FindPackagesByUserSecurityId(&HSTRING::new())? {
                let id = package.Id()?;
                let name = id.Name()?.to_string();
                // Package names are discovered from Windows, not a guessed source AUMID.
                if name.starts_with("AppleInc.") && name.contains("AppleMusic") {
                    families.push(id.FamilyName()?.to_string());
                }
            }
        }
        tracing::info!(
            packages = families.len(),
            explicit_source = explicit.is_some(),
            "Windows media provider ready"
        );
        let mut provider = Self {
            manager,
            sessions_token: None,
            current_token: None,
            selected: None,
            families,
            explicit,
            tx,
            generation: 0,
            cache: MusicInfo::default(),
            metadata_key: String::new(),
            timeline_stamp: None,
        };
        let tx = provider.tx.clone();
        provider.sessions_token = Some(provider.manager.SessionsChanged(
            &TypedEventHandler::new(move |_, _| {
                let _ = tx.try_send(WorkerRequest::Refresh {
                    generation: None,
                    artwork: true,
                });
                Ok(())
            }),
        )?);
        let tx = provider.tx.clone();
        provider.current_token = Some(provider.manager.CurrentSessionChanged(
            &TypedEventHandler::new(move |_, _| {
                let _ = tx.try_send(WorkerRequest::Refresh {
                    generation: None,
                    artwork: false,
                });
                Ok(())
            }),
        )?);
        Ok(provider)
    }

    fn discover(&mut self) -> Result<bool, MediaError> {
        let sessions = self.manager.GetSessions()?;
        let current = self.manager.GetCurrentSession().ok();
        let mut candidates = Vec::new();
        for session in sessions {
            let source = session.SourceAppUserModelId()?.to_string();
            tracing::debug!(source = %source, "Discovered GSMTC source");
            if matches_source(&source, &self.families, self.explicit.as_deref()) {
                candidates.push(session);
            }
        }
        let chosen = candidates
            .iter()
            .find(|s| Some(*s) == current.as_ref())
            .or_else(|| {
                candidates
                    .iter()
                    .find(|s| self.selected.as_ref().is_some_and(|old| old.session == **s))
            })
            .or_else(|| candidates.first())
            .cloned();
        if chosen.as_ref() == self.selected.as_ref().map(|s| &s.session) {
            return Ok(false);
        }
        self.selected.take(); // Detach all old callbacks before replacing the session.
        self.generation += 1;
        self.cache = MusicInfo::default();
        self.metadata_key.clear();
        self.timeline_stamp = None;
        let Some(session) = chosen else {
            tracing::info!("Apple Music session unavailable");
            return Ok(true);
        };
        let generation = self.generation;
        let mut bound = BoundSession {
            session,
            media: None,
            playback: None,
            timeline: None,
        };
        let tx = self.tx.clone();
        bound.media = Some(
            bound
                .session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = tx.try_send(WorkerRequest::Refresh {
                        generation: Some(generation),
                        artwork: true,
                    });
                    Ok(())
                }))?,
        );
        let tx = self.tx.clone();
        bound.playback = Some(bound.session.PlaybackInfoChanged(&TypedEventHandler::new(
            move |_, _| {
                let _ = tx.try_send(WorkerRequest::Refresh {
                    generation: Some(generation),
                    artwork: false,
                });
                Ok(())
            },
        ))?);
        let tx = self.tx.clone();
        bound.timeline = Some(
            bound
                .session
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = tx.try_send(WorkerRequest::Refresh {
                        generation: Some(generation),
                        artwork: false,
                    });
                    Ok(())
                }))?,
        );
        tracing::info!(source = %bound.session.SourceAppUserModelId()?, generation, "Selected Apple Music session");
        self.selected = Some(bound);
        Ok(true)
    }
}

impl MediaProvider for WindowsMediaProvider {
    fn snapshot(&mut self, refresh_artwork: bool) -> Result<MusicInfo, MediaError> {
        let changed = self.discover()?;
        let Some(bound) = &self.selected else {
            return Ok(MusicInfo::default());
        };
        let session = &bound.session;
        let properties = session.TryGetMediaPropertiesAsync()?.join()?;
        let title = properties.Title()?.to_string();
        let artist = properties.Artist()?.to_string();
        let album = properties.AlbumTitle()?.to_string();
        let key = format!("{title}\0{artist}\0{album}");
        let track_changed = changed || key != self.metadata_key;
        let artwork = if track_changed || refresh_artwork {
            match read_artwork(&properties) {
                Ok(art) => art,
                Err(error) => {
                    tracing::debug!(%error, "Artwork unavailable");
                    None
                }
            }
        } else {
            self.cache.artwork.clone()
        };
        self.metadata_key = key;
        let playback = session.GetPlaybackInfo()?;
        let status = playback.PlaybackStatus()?;
        let controls = playback.Controls()?;
        let mut capabilities = Capabilities {
            play: controls.IsPlayEnabled()?,
            pause: controls.IsPauseEnabled()?,
            toggle: controls.IsPlayPauseToggleEnabled()?,
            previous: controls.IsPreviousEnabled()?,
            next: controls.IsNextEnabled()?,
            seek: controls.IsPlaybackPositionEnabled()?,
            shuffle: controls.IsShuffleEnabled()?,
            repeat: controls.IsRepeatEnabled()?,
        };
        let rate = playback
            .PlaybackRate()
            .and_then(|r| r.Value())
            .unwrap_or(1.0);
        let playing = status == Status::Playing;
        let timeline = session
            .GetTimelineProperties()
            .ok()
            .map(|t| -> Result<Timeline, MediaError> {
                let seconds = |ticks: i64| ticks as f64 / 10_000_000.0;
                let start = seconds(t.StartTime()?.Duration);
                let end = seconds(t.EndTime()?.Duration).max(start);
                let updated_ticks = t.LastUpdatedTime()?.UniversalTime;
                let position_ticks = t.Position()?.Duration;
                let updated = seconds(updated_ticks) - 11_644_473_600.0;
                let wall_now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
                let now = Instant::now();
                let rate = if rate.is_finite() { rate } else { 1.0 };
                let stamp = (updated_ticks, position_ticks);
                // Some players keep LastUpdatedTime unchanged while paused. Never
                // turn the entire paused wall-clock interval into played audio on resume.
                let position = if !track_changed && self.timeline_stamp == Some(stamp) {
                    self.cache
                        .timeline
                        .as_ref()
                        .map_or(seconds(position_ticks), |previous| {
                            previous.start + previous.elapsed(now, self.cache.playing) as f64
                        })
                } else {
                    let age = if !track_changed
                        && self.cache.available
                        && self.cache.playing
                        && playing
                    {
                        (wall_now - updated).max(0.0) * rate
                    } else {
                        0.0
                    };
                    seconds(position_ticks) + age
                }
                .clamp(start, end);
                self.timeline_stamp = Some(stamp);
                Ok(Timeline {
                    start,
                    end,
                    position,
                    rate,
                    min_seek: seconds(t.MinSeekTime()?.Duration),
                    max_seek: seconds(t.MaxSeekTime()?.Duration),
                    captured_at: now,
                })
            })
            .transpose()?;
        capabilities.seek &= timeline.as_ref().is_some_and(|t| {
            t.end > t.start
                && t.max_seek > t.min_seek
                && t.max_seek >= t.start
                && t.min_seek <= t.end
        });
        let repeat_value = playback.AutoRepeatMode().and_then(|v| v.Value()).ok();
        let shuffle_value = playback.IsShuffleActive().and_then(|v| v.Value()).ok();
        capabilities.repeat &= repeat_value.is_some();
        capabilities.shuffle &= shuffle_value.is_some();
        let music = MusicInfo {
            title,
            artist,
            album,
            source: session.SourceAppUserModelId()?.to_string(),
            artwork,
            generation: self.generation,
            available: true,
            playing,
            stopped: status == Status::Stopped || status == Status::Closed,
            duration: timeline.as_ref().map_or(0.0, Timeline::duration),
            position: timeline
                .as_ref()
                .map_or(0.0, |t| t.elapsed(Instant::now(), playing)),
            timeline,
            capabilities,
            shuffle: shuffle_value.unwrap_or(false),
            repeat: match repeat_value {
                Some(MediaPlaybackAutoRepeatMode::Track) => Repeat::One,
                Some(MediaPlaybackAutoRepeatMode::List) => Repeat::All,
                _ => Repeat::Off,
            },
            ..MusicInfo::default()
        };
        tracing::debug!(
            playing,
            artwork = music.artwork.is_some(),
            ?capabilities,
            timeline = music.timeline.is_some(),
            "Media snapshot"
        );
        self.cache = music.clone();
        Ok(music)
    }

    fn command(&mut self, request: MediaRequest) -> Result<(), MediaError> {
        if self.discover()? || request.generation != self.generation {
            return Err(MediaError::StaleSession);
        }
        if !self.cache.supports(request.command) {
            return Err(MediaError::Unsupported);
        }
        let session = &self
            .selected
            .as_ref()
            .ok_or(MediaError::Unavailable)?
            .session;
        let operation = match request.command {
            MediaCommand::Play => session.TryPlayAsync()?,
            MediaCommand::Pause => session.TryPauseAsync()?,
            MediaCommand::PlayPause if self.cache.capabilities.toggle => {
                session.TryTogglePlayPauseAsync()?
            }
            MediaCommand::PlayPause if self.cache.playing => session.TryPauseAsync()?,
            MediaCommand::PlayPause => session.TryPlayAsync()?,
            MediaCommand::Next => session.TrySkipNextAsync()?,
            MediaCommand::Previous => session.TrySkipPreviousAsync()?,
            MediaCommand::Shuffle => session.TryChangeShuffleActiveAsync(!self.cache.shuffle)?,
            MediaCommand::Repeat => {
                session.TryChangeAutoRepeatModeAsync(match self.cache.repeat {
                    Repeat::Off => MediaPlaybackAutoRepeatMode::List,
                    Repeat::All => MediaPlaybackAutoRepeatMode::Track,
                    Repeat::One => MediaPlaybackAutoRepeatMode::None,
                })?
            }
            MediaCommand::Seek(ratio) => session.TryChangePlaybackPositionAsync(
                self.cache
                    .timeline
                    .as_ref()
                    .and_then(|t| t.seek_ticks(ratio))
                    .ok_or(MediaError::Unsupported)?,
            )?,
        };
        if operation.join()? {
            Ok(())
        } else {
            Err(MediaError::Rejected)
        }
    }
}

fn read_artwork(
    properties: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Result<Option<iced::widget::image::Handle>, MediaError> {
    let Ok(reference) = properties.Thumbnail() else {
        return Ok(None);
    };
    let stream = reference.OpenReadAsync()?.join()?;
    let size = stream.Size()?;
    if size == 0 || size > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream)?;
    let loaded = reader.LoadAsync(size as u32)?.join()?;
    let mut bytes = vec![0; loaded as usize];
    reader.ReadBytes(&mut bytes)?;
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| MediaError::Native(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    decoder.limits(limits);
    let mut image = decoder
        .decode()
        .map_err(|e| MediaError::Native(e.to_string()))?
        .resize_to_fill(208, 208, image::imageops::FilterType::Triangle)
        .to_rgba8();
    crate::services::artwork::round_corners(&mut image);
    let _ = stream.Close();
    Ok(Some(iced::widget::image::Handle::from_rgba(
        208,
        208,
        image.into_raw(),
    )))
}

pub fn subscription(source: Option<String>) -> Subscription<Message> {
    Subscription::run_with(source, |source| {
        let source = source.clone();
        iced::stream::channel(
            16,
            move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let (tx, rx) = mpsc::sync_channel(32);
                let (updates, mut incoming) = tokio::sync::mpsc::channel(8);
                let worker_tx = tx.clone();
                let _ = std::thread::Builder::new()
                    .name("rimflux-media".into())
                    .spawn(move || {
                        let _apartment = match Apartment::new() {
                            Ok(a) => a,
                            Err(e) => {
                                let _ =
                                    updates.blocking_send(Message::MediaSnapshot(unavailable(e)));
                                return;
                            }
                        };
                        let mut provider = None;
                        loop {
                            if updates.is_closed() {
                                break;
                            }
                            if provider.is_none() {
                                match WindowsMediaProvider::new(worker_tx.clone(), source.clone()) {
                                    Ok(p) => provider = Some(p),
                                    Err(e) => {
                                        let _ = updates
                                            .blocking_send(Message::MediaSnapshot(unavailable(e)));
                                    }
                                }
                            }
                            if let Some(p) = provider.as_mut() {
                                let snapshot = p.snapshot(false).unwrap_or_else(unavailable);
                                if updates
                                    .blocking_send(Message::MediaSnapshot(snapshot))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            match rx.recv_timeout(Duration::from_secs(5)) {
                                Ok(WorkerRequest::Command(request)) => {
                                    let result = provider
                                        .as_mut()
                                        .ok_or(MediaError::Unavailable)
                                        .and_then(|p| p.command(request));
                                    if updates
                                        .blocking_send(Message::MediaCommandResult(
                                            result.map_err(|e| e.to_string()),
                                        ))
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Ok(WorkerRequest::Refresh {
                                    generation,
                                    artwork,
                                }) => {
                                    if let Some(p) = provider.as_mut()
                                        && generation.is_none_or(|g| g == p.generation)
                                    {
                                        let snapshot =
                                            p.snapshot(artwork).unwrap_or_else(unavailable);
                                        if updates
                                            .blocking_send(Message::MediaSnapshot(snapshot))
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    });
                if output.send(Message::MediaConnected(tx)).await.is_err() {
                    return;
                }
                while let Some(update) = incoming.recv().await {
                    if output.send(update).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}

fn unavailable(error: MediaError) -> MusicInfo {
    tracing::warn!(%error, "Media provider unavailable");
    MusicInfo {
        error: Some(error.to_string()),
        ..MusicInfo::default()
    }
}

pub fn probe(source: Option<String>) -> Result<(), MediaError> {
    let _apartment = Apartment::new()?;
    let (tx, _) = mpsc::sync_channel(32);
    let mut provider = WindowsMediaProvider::new(tx, source)?;
    let music = provider.snapshot(false)?;
    println!(
        "available={} source={:?} playing={} title={:?} artist={:?} artwork={} timeline={} controls={:?}",
        music.available,
        music.source,
        music.playing,
        music.title,
        music.artist,
        music.artwork.is_some(),
        music.timeline.is_some(),
        music.capabilities
    );
    Ok(())
}
