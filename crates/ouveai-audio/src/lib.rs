//! Captura de áudio do OuveAI (Windows, WASAPI): EU = microfone, OUTROS = loopback.

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod device;
pub mod timeline;

#[cfg(windows)]
pub use capture::{Capture, Ch, Config, Event, EventSink, MicMode, OutrosMode, RATE, Sink, pos_of, qpc_100ns};
#[cfg(windows)]
pub use device::{DeviceInfo, default_output_is_headphone, list_devices, playing_apps};
