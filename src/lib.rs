pub mod audio;
#[cfg(target_os = "linux")]
pub mod bluetooth;
#[cfg(target_os = "macos")]
#[path = "bluetooth_macos.rs"]
pub mod bluetooth;
#[cfg(windows)]
#[path = "bluetooth_windows.rs"]
pub mod bluetooth;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[path = "bluetooth_unsupported.rs"]
pub mod bluetooth;
pub mod build;
pub mod changes;
pub mod completion;
pub mod device_info;
pub mod download;
pub mod fota;
pub mod generate;
pub mod guard;
pub mod keys;
pub mod language;
pub mod link;
pub mod manifest;
pub mod mdr;
pub mod pack;
pub mod race;
pub mod size;
pub mod slots;
pub mod source;
pub mod upload;
pub mod workflow;
pub mod workspace;

/// An error for input that is not in the format Sony uses.
fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}
