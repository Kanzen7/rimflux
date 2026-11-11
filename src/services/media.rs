use crate::modules::music::{MediaCommand, MusicInfo};

#[derive(Debug, Clone, thiserror::Error)]
pub enum MediaError {
    #[error("Apple Music has no exposed Windows media session")]
    Unavailable,
    #[error("this playback operation is not supported")]
    Unsupported,
    #[error("the selected media session changed; please retry")]
    StaleSession,
    #[error("the media application rejected the operation")]
    Rejected,
    #[error("Windows media service: {0}")]
    Native(String),
}

#[derive(Debug, Clone, Copy)]
pub struct MediaRequest {
    pub generation: u64,
    pub command: MediaCommand,
}

/// Only owned snapshots and commands cross this boundary; never native session objects.
pub trait MediaProvider {
    fn snapshot(&mut self, refresh_artwork: bool) -> Result<MusicInfo, MediaError>;
    fn command(&mut self, request: MediaRequest) -> Result<(), MediaError>;
}

pub struct MockMediaProvider {
    pub music: MusicInfo,
}
impl Default for MockMediaProvider {
    fn default() -> Self {
        Self {
            music: MusicInfo::demo(),
        }
    }
}
impl MediaProvider for MockMediaProvider {
    fn snapshot(&mut self, _: bool) -> Result<MusicInfo, MediaError> {
        Ok(self.music.clone())
    }
    fn command(&mut self, request: MediaRequest) -> Result<(), MediaError> {
        self.music.command(request.command);
        Ok(())
    }
}

/// Compare package-family identity, never a loose substring in a browser source ID.
pub fn matches_source(source: &str, families: &[String], explicit: Option<&str>) -> bool {
    if let Some(explicit) = explicit {
        return source.eq_ignore_ascii_case(explicit);
    }
    let family = source.split('!').next().unwrap_or(source);
    families
        .iter()
        .any(|known| family.eq_ignore_ascii_case(known))
}
