//! coreaudio-rs
//! ------------
//!
//! A friendly rust interface for Apple's CoreAudio API.
//!
//! Read the CoreAudio Overview [here](https://developer.apple.com/library/mac/documentation/MusicAudio/Conceptual/CoreAudioOverview/Introduction/Introduction.html).
//!
//! Currently, work has only been started on the [audio_unit](./audio_unit/index.html) module, but
//! eventually we'd like to cover at least the majority of the C API.

#[macro_use]
extern crate bitflags;
extern crate core_foundation_sys;
pub extern crate coreaudio_sys as sys;

pub use error::Error;

#[cfg(feature = "audio_unit")]
pub mod audio_unit;

#[cfg(feature = "audio_queue")]
pub mod audio_queue;

pub mod error;

mod audio_format;
pub use audio_format::*;

mod sample_format;
pub use sample_format::*;

mod stream_format;
pub use stream_format::*;

macro_rules! try_os_status {
    ($expr:expr) => {
        Error::from_os_status($expr)?
    };
}
pub(crate) use try_os_status;
