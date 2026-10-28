//! Any other OS: there is no device link. Download, generate and build still work; upload and
//! device-info report where the link works.

use std::io;
use std::time::Duration;

use crate::link::Transport;

/// An SDP service UUID.
pub type Uuid = u128;

/// Sony's MDR control channel, `Serial HPC` in SDP.
pub const MDR_UUID: Uuid = 0x96cc203e_5068_46ad_b32d_e316f5e069ba;
/// Airoha's RACE channel, `Airoha_APP` in SDP.
pub const RACE_UUID: Uuid = 0x8901dfa8_5c7e_4d8f_9f0c_c2b70683f5f0;

pub struct Headphones {
    pub name: String,
    pub address: String,
}

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "talking to the headphones over Bluetooth works on Linux, macOS and Windows only",
    )
}

impl Headphones {
    pub fn find() -> io::Result<Self> {
        Err(unsupported())
    }

    pub fn open(&self, _uuid: Uuid, _label: &'static str, _verbose: bool) -> io::Result<Channel> {
        Err(unsupported())
    }
}

pub struct Channel;

impl Transport for Channel {
    fn send(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Err(unsupported())
    }

    fn receive(&mut self, _timeout: Duration) -> io::Result<Vec<u8>> {
        Err(unsupported())
    }
}
