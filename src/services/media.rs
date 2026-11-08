use crate::modules::music::{MediaCommand, MusicInfo};
use std::time::Duration;

/// OS media providers can replace this interface without depending on UI widgets.
pub trait MediaProvider {
    fn advance(&mut self, state: &mut MusicInfo, elapsed: Duration);
    fn command(&mut self, state: &mut MusicInfo, command: MediaCommand);
}

pub struct MockMedia;

impl MediaProvider for MockMedia {
    fn advance(&mut self, state: &mut MusicInfo, elapsed: Duration) {
        state.advance(elapsed);
    }
    fn command(&mut self, state: &mut MusicInfo, command: MediaCommand) {
        state.command(command);
    }
}
