//! Event-driven, shared-mode output loopback. COM objects and transient samples
//! stay on this worker. No microphone, file output, or protected-content workaround.
use crate::{
    message::Message,
    services::audio::{
        AnalysisSettings, AudioAnalyzer, AudioCaptureStatus, AudioSample, AudioSnapshot,
    },
};
use iced::{Subscription, futures::SinkExt};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Media::{
            Audio::{
                AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
                AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient, IMMDevice,
                IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
                eConsole, eRender,
            },
            KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE},
            Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT},
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
    core::PCWSTR,
};

#[derive(Debug, thiserror::Error)]
enum CaptureError {
    #[error("WASAPI: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("{0}")]
    Unsupported(&'static str),
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, CaptureError> {
        // SAFETY: this dedicated thread owns and balances its COM apartment.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct EventHandle(HANDLE);
impl Drop for EventHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct FormatMemory(*mut WAVEFORMATEX);
impl Drop for FormatMemory {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.cast()));
        }
    }
}

#[derive(Clone, Copy)]
enum Encoding {
    Float32,
    Float64,
    Pcm8,
    Pcm16,
    Pcm24,
    Pcm32,
}
struct AudioFormat {
    rate: u32,
    channels: usize,
    block: usize,
    sample_bytes: usize,
    encoding: Encoding,
}
impl AudioFormat {
    unsafe fn read(pointer: *const WAVEFORMATEX) -> Result<Self, CaptureError> {
        if pointer.is_null() {
            return Err(CaptureError::Unsupported("Empty endpoint mix format"));
        }
        // SAFETY: GetMixFormat owns this allocation through FormatMemory. Extended
        // fields are read only after checking cbSize; packed values use unaligned reads.
        let format = unsafe { pointer.read_unaligned() };
        let floating = if format.wFormatTag as u32 == WAVE_FORMAT_EXTENSIBLE {
            if format.cbSize < 22 {
                return Err(CaptureError::Unsupported("Truncated extended mix format"));
            }
            let subtype = unsafe {
                std::ptr::addr_of!((*(pointer.cast::<WAVEFORMATEXTENSIBLE>())).SubFormat)
                    .read_unaligned()
            };
            if subtype == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
                true
            } else if subtype == KSDATAFORMAT_SUBTYPE_PCM {
                false
            } else {
                return Err(CaptureError::Unsupported(
                    "Unsupported extended audio subtype",
                ));
            }
        } else if format.wFormatTag as u32 == WAVE_FORMAT_IEEE_FLOAT {
            true
        } else if format.wFormatTag == 1 {
            false
        } else {
            return Err(CaptureError::Unsupported("Unsupported endpoint mix format"));
        };
        let encoding = match (floating, format.wBitsPerSample) {
            (true, 32) => Encoding::Float32,
            (true, 64) => Encoding::Float64,
            (false, 8) => Encoding::Pcm8,
            (false, 16) => Encoding::Pcm16,
            (false, 24) => Encoding::Pcm24,
            (false, 32) => Encoding::Pcm32,
            _ => return Err(CaptureError::Unsupported("Unsupported audio sample size")),
        };
        let rate = format.nSamplesPerSec;
        let channels = format.nChannels as usize;
        let sample_bytes = format.wBitsPerSample as usize / 8;
        let block = format.nBlockAlign as usize;
        if channels == 0
            || channels > 32
            || block < channels * sample_bytes
            || block > 1024
            || !(8000..=192000).contains(&rate)
        {
            return Err(CaptureError::Unsupported(
                "Endpoint format exceeds bounded analysis limits",
            ));
        }
        Ok(Self {
            rate,
            channels,
            block,
            sample_bytes,
            encoding,
        })
    }
    fn decode(&self, frame: &[u8]) -> AudioSample {
        let mut mono = 0.0;
        let mut power = 0.0;
        for channel in 0..self.channels {
            let at = channel * self.sample_bytes;
            let b = &frame[at..at + self.sample_bytes];
            let value = match self.encoding {
                Encoding::Float32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                Encoding::Float64 => {
                    f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f32
                }
                Encoding::Pcm8 => (b[0] as f32 - 128.0) / 128.0,
                Encoding::Pcm16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
                Encoding::Pcm24 => {
                    ((i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32) / 8388608.0
                }
                Encoding::Pcm32 => {
                    i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0
                }
            };
            let value = if value.is_finite() {
                value.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            mono += value;
            power += value * value;
        }
        AudioSample {
            mono: mono / self.channels as f32,
            power: power / self.channels as f32,
        }
    }
}

struct Loopback {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    enumerator: IMMDeviceEnumerator,
    endpoint_id: String,
    event: EventHandle,
    format: AudioFormat,
    packet: Vec<AudioSample>,
    buffer_frames: usize,
}
impl Drop for Loopback {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
impl Loopback {
    fn open() -> Result<Self, CaptureError> {
        // SAFETY: all interfaces and handles remain on this initialized COM thread.
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let endpoint = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let endpoint_id = device_id(&endpoint)?;
            let client: IAudioClient = endpoint.Activate(CLSCTX_ALL, None)?;
            let memory = FormatMemory(client.GetMixFormat()?);
            let format = AudioFormat::read(memory.0)?;
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                1_000_000,
                0,
                memory.0,
                None,
            )?;
            let buffer_frames = client.GetBufferSize()? as usize;
            if buffer_frames == 0 || buffer_frames > 65536 {
                return Err(CaptureError::Unsupported(
                    "Endpoint buffer exceeds bounded analysis limit",
                ));
            }
            let capture: IAudioCaptureClient = client.GetService()?;
            let event = EventHandle(CreateEventW(None, false, false, PCWSTR::null())?);
            client.SetEventHandle(event.0)?;
            client.Start()?;
            Ok(Self {
                client,
                capture,
                enumerator,
                endpoint_id,
                event,
                format,
                packet: Vec::with_capacity(buffer_frames),
                buffer_frames,
            })
        }
    }

    fn endpoint_unchanged(&self) -> Result<bool, CaptureError> {
        let endpoint = unsafe { self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
        Ok(device_id(&endpoint)? == self.endpoint_id)
    }

    fn drain(&mut self, analyzer: &mut AudioAnalyzer, now: Instant) -> Result<bool, CaptureError> {
        let mut received = false;
        // Bound the number of packets per wake so cancellation cannot be starved.
        for _ in 0..64 {
            if unsafe { self.capture.GetNextPacketSize()? } == 0 {
                break;
            }
            let mut data = std::ptr::null_mut();
            let mut frames = 0_u32;
            let mut flags = 0_u32;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;
            }
            if frames == 0 {
                break;
            }
            // PacketGuard guarantees GetBuffer/ReleaseBuffer pairing on this thread,
            // even when format validation or decoding cannot proceed.
            let guard = PacketGuard {
                capture: &self.capture,
                frames,
            };
            if frames as usize > self.buffer_frames {
                return Err(CaptureError::Unsupported("Oversized loopback packet"));
            }
            if flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0 {
                analyzer.reset_window();
            }
            self.packet.clear();
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                self.packet.resize(frames as usize, AudioSample::default());
            } else {
                if data.is_null() {
                    return Err(CaptureError::Unsupported("Missing loopback packet data"));
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(data, frames as usize * self.format.block)
                };
                for frame in bytes.chunks_exact(self.format.block) {
                    self.packet.push(self.format.decode(frame));
                }
            }
            drop(guard); // Release the native packet before FFT and onset analysis.
            for sample in &self.packet {
                analyzer.push(*sample, now);
            }
            self.packet.fill(AudioSample::default());
            self.packet.clear();
            received = true;
        }
        Ok(received)
    }
}

struct PacketGuard<'a> {
    capture: &'a IAudioCaptureClient,
    frames: u32,
}
impl Drop for PacketGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.capture.ReleaseBuffer(self.frames);
        }
    }
}

fn device_id(endpoint: &IMMDevice) -> Result<String, CaptureError> {
    unsafe {
        let pointer = endpoint.GetId()?;
        let id = pointer.to_string();
        CoTaskMemFree(Some(pointer.0.cast()));
        id.map_err(|_| CaptureError::Unsupported("Invalid output endpoint identifier"))
    }
}

struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
static CAPTURE_OWNER: Mutex<()> = Mutex::new(());

/// Bounded, read-only diagnostics. Uses the normal capture worker, exports scalars only.
#[cfg(debug_assertions)]
pub fn probe(
    config: &crate::core::config::WaveformConfig,
    duration: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::modules::traveling_wave::{TravelingInput, TravelingLayer, TravelingWaves};
    use crate::modules::waveform_reactivity::{MusicalLayer, WaveformReactivity};
    let settings = AnalysisSettings::from(config);
    let stop = Arc::new(AtomicBool::new(false));
    let guard = StopOnDrop(stop.clone());
    let (updates, incoming) =
        tokio::sync::watch::channel(AudioSnapshot::unavailable("Starting probe"));
    let worker = std::thread::Builder::new()
        .name("rimflux-audio-probe".into())
        .spawn(move || run(stop, updates, settings))?;
    let start = Instant::now();
    let mut previous = start;
    let mut logged = start;
    let mut last_capture = None;
    let mut live = 0;
    let mut minima = [1.0_f32; 8];
    let mut maxima = [0.0_f32; 8];
    let mut amplitude_min = [f32::MAX; 5];
    let mut amplitude_max = [0.0_f32; 5];
    let mut layers = WaveformReactivity::default();
    let mut traveling = TravelingWaves::default();
    let mut velocity_min = [f64::MAX; 3];
    let mut velocity_max = [0.0_f64; 3];
    let mut ripple_peak = 0.0_f32;
    while start.elapsed() < duration.min(Duration::from_secs(30)) {
        std::thread::sleep(Duration::from_millis(10));
        let now = Instant::now();
        let s = incoming.borrow().clone();
        let fresh = s.status == AudioCaptureStatus::Live
            && now.saturating_duration_since(s.captured_at) < Duration::from_millis(350);
        let f = s.features;
        if fresh {
            layers.ingest(&s.transients, now, start);
            if Some(s.captured_at) != last_capture {
                live += 1;
                last_capture = Some(s.captured_at);
                for ((minimum, maximum), value) in minima.iter_mut().zip(&mut maxima).zip([
                    f.overall_energy,
                    f.bass_energy,
                    f.mid_energy,
                    f.presence_energy,
                    f.high_energy,
                    f.low_onset,
                    f.mid_onset,
                    f.high_onset,
                ]) {
                    *minimum = minimum.min(value);
                    *maximum = maximum.max(value);
                }
            }
        }
        let elapsed = now.saturating_duration_since(previous);
        layers.advance(
            elapsed.as_secs_f32(),
            now,
            if fresh { f } else { Default::default() },
            fresh,
        );
        traveling.advance(
            elapsed,
            TravelingInput {
                features: layers.features,
                blend: if fresh && config.reactivity.enabled {
                    1.0
                } else {
                    0.0
                },
                onset_density: layers.onset_density(now),
                activity: layers.activity(now),
                playing: true,
                animation_speed: config.animation_speed,
            },
        );
        let velocities =
            TravelingLayer::ALL.map(|layer| traveling.parameters(layer).angular_velocity);
        previous = now;
        let amplitudes = [
            MusicalLayer::Bass,
            MusicalLayer::Mid,
            MusicalLayer::Percussion,
            MusicalLayer::High,
            MusicalLayer::Ambient,
        ]
        .map(|role| {
            layers
                .response(role, &config.reactivity, 1.0, now)
                .amplitude
        });
        if fresh && start.elapsed() > Duration::from_secs(1) {
            for ((minimum, maximum), value) in velocity_min
                .iter_mut()
                .zip(&mut velocity_max)
                .zip(velocities)
            {
                *minimum = minimum.min(value);
                *maximum = maximum.max(value);
            }
            for ((minimum, maximum), value) in amplitude_min
                .iter_mut()
                .zip(&mut amplitude_max)
                .zip(amplitudes)
            {
                *minimum = minimum.min(value);
                *maximum = maximum.max(value);
            }
        }
        ripple_peak = ripple_peak.max(layers.activity(now));
        if now.duration_since(logged) >= Duration::from_millis(250) {
            logged = now;
            tracing::info!(target: "rimflux::audio_probe", capture=?s.status, bass=f.bass_energy, mid=f.mid_energy, presence=f.presence_energy, high=f.high_energy, low_onset=f.low_onset, mid_onset=f.mid_onset, high_onset=f.high_onset, ripple_activity=layers.activity(now), ?amplitudes, ?velocities, reason=?s.reason, "Live system-output features");
        }
    }
    drop(guard);
    drop(incoming);
    worker
        .join()
        .map_err(|_| "Audio diagnostic worker panicked")?;
    tracing::info!(target: "rimflux::audio_probe", live_snapshots=live, ?minima, ?maxima, ?amplitude_min, ?amplitude_max, ?velocity_min, ?velocity_max, transients=layers.transient_count(), ripple_peak, "Multiband probe summary (scalar features only)");
    Ok(())
}

pub fn subscription(settings: AnalysisSettings) -> Subscription<Message> {
    Subscription::run_with(settings, |settings| {
        let settings = *settings;
        iced::stream::channel(
            2,
            move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                let stop = Arc::new(AtomicBool::new(false));
                let _guard = StopOnDrop(stop.clone());
                let (updates, mut incoming) = tokio::sync::watch::channel(AudioSnapshot {
                    status: AudioCaptureStatus::Starting,
                    ..AudioSnapshot::unavailable("Starting output loopback")
                });
                let worker = std::thread::Builder::new()
                    .name("rimflux-loopback".into())
                    .spawn(move || run(stop, updates, settings));
                if let Err(error) = worker {
                    let _ = output
                        .send(Message::AudioSnapshot(AudioSnapshot::unavailable(
                            error.to_string(),
                        )))
                        .await;
                    return;
                }
                while incoming.changed().await.is_ok() {
                    let snapshot = incoming.borrow_and_update().clone();
                    if output.send(Message::AudioSnapshot(snapshot)).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}

fn run(
    stop: Arc<AtomicBool>,
    updates: tokio::sync::watch::Sender<AudioSnapshot>,
    settings: AnalysisSettings,
) {
    let cancelled = || stop.load(Ordering::Acquire) || updates.is_closed();
    // At most one open loopback stream, even during rapid subscription replacement.
    let _owner = loop {
        if cancelled() {
            return;
        }
        match CAPTURE_OWNER.try_lock() {
            Ok(owner) => break owner,
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(_) => {
                updates.send_replace(AudioSnapshot::unavailable("Loopback worker unavailable"));
                return;
            }
        }
    };
    let _apartment = match Apartment::new() {
        Ok(apartment) => apartment,
        Err(error) => {
            updates.send_replace(AudioSnapshot::unavailable(error.to_string()));
            return;
        }
    };
    let mut last_error = String::new();
    while !cancelled() {
        let result = (|| -> Result<(), CaptureError> {
            let mut loopback = Loopback::open()?;
            let mut analyzer = AudioAnalyzer::new(loopback.format.rate, settings);
            tracing::info!(
                sample_rate = loopback.format.rate,
                "WASAPI output loopback connected; system-mix features only"
            );
            let mut last_packet = Instant::now();
            let mut last_publish = Instant::now();
            let mut last_published_onset = None;
            let mut last_published_transient = None;
            let mut last_device_check = Instant::now();
            let mut silent = false;
            while !cancelled() {
                let event = unsafe { WaitForSingleObject(loopback.event.0, 100) };
                if event != WAIT_OBJECT_0 && event != WAIT_TIMEOUT {
                    return Err(CaptureError::Unsupported("Loopback event wait failed"));
                }
                let now = Instant::now();
                if loopback.drain(&mut analyzer, now)? {
                    last_packet = now;
                    silent = false;
                } else if now.duration_since(last_packet) > Duration::from_millis(150) {
                    if !silent {
                        analyzer.reset_window();
                        silent = true;
                    }
                    analyzer.silence(now);
                }
                if now.duration_since(last_publish) >= Duration::from_millis(33)
                    || analyzer.snapshot.onset_at != last_published_onset
                    || analyzer.snapshot.transients[0].map(|event| event.at)
                        != last_published_transient
                {
                    updates.send_replace(analyzer.snapshot.clone());
                    last_published_onset = analyzer.snapshot.onset_at;
                    last_published_transient =
                        analyzer.snapshot.transients[0].map(|event| event.at);
                    last_publish = now;
                }
                if now.duration_since(last_device_check) >= Duration::from_secs(1) {
                    if !loopback.endpoint_unchanged()? {
                        return Err(CaptureError::Unsupported("Default output endpoint changed"));
                    }
                    last_device_check = now;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            let reason = error.to_string();
            if reason != last_error {
                tracing::warn!(%reason,"Audio-reactive input unavailable; procedural fallback");
                last_error = reason.clone();
            }
            updates.send_replace(AudioSnapshot::unavailable(reason));
        }
        if cancelled() {
            break;
        }
        // Retry transient device/service failures without spinning or blocking the UI.
        for _ in 0..50 {
            if cancelled() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
