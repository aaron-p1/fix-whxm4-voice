//! Windows: RFCOMM channels to the headphones through Winsock's Bluetooth sockets.
//!
//! Windows finds each channel by its SDP UUID: a Bluetooth socket connected to a device address and
//! a service class UUID, with no channel number, makes Windows look the channel up over SDP and
//! connect. The headphones are the connected device whose SDP records, as Windows cached them when
//! pairing, offer Sony's MDR channel; Winsock's name-space API reads the cache. See ADR 0007.
//!
//! The sockets block. A reply is awaited with `select`, so that a timeout leaves the socket
//! untouched: Winsock says a socket whose receive timed out through `SO_RCVTIMEO` should be closed.

use std::fmt;
use std::io;
use std::ptr;
use std::sync::OnceLock;
use std::time::Duration;

use windows_sys::Win32::Devices::Bluetooth::{
    AF_BTH, BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_FIND_RADIO_PARAMS,
    BTHPROTO_RFCOMM, BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindFirstRadio,
    BluetoothFindNextDevice, BluetoothFindRadioClose, NS_BTH, SOCKADDR_BTH,
};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, HANDLE};
use windows_sys::Win32::Networking::WinSock::{
    FD_SET, INVALID_SOCKET, LUP_RETURN_ADDR, SOCK_STREAM, SOCKADDR, SOCKET, SOCKET_ERROR, TIMEVAL,
    WSA_E_NO_MORE, WSADATA, WSAEDISCON, WSAEHOSTDOWN, WSAEHOSTUNREACH, WSAENETDOWN, WSAENETRESET,
    WSAENETUNREACH, WSAENOMORE, WSAESHUTDOWN, WSAGetLastError, WSALookupServiceBeginW,
    WSALookupServiceEnd, WSALookupServiceNextW, WSAQUERYSETW, WSASERVICE_NOT_FOUND, WSAStartup,
    closesocket, connect, recv, select, send, socket,
};
use windows_sys::core::GUID;

use crate::link::{Transport, hex};

/// An SDP service UUID.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Uuid(u128);

impl Uuid {
    pub const fn from_u128(value: u128) -> Self {
        Uuid(value)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{:032x}", self.0);
        write!(
            f,
            "{}-{}-{}-{}-{}",
            &s[0..8],
            &s[8..12],
            &s[12..16],
            &s[16..20],
            &s[20..32]
        )
    }
}

/// Sony's MDR control channel, `Serial HPC` in SDP.
pub const MDR_UUID: Uuid = Uuid::from_u128(0x96cc203e_5068_46ad_b32d_e316f5e069ba);
/// Airoha's RACE channel, `Airoha_APP` in SDP.
pub const RACE_UUID: Uuid = Uuid::from_u128(0x8901dfa8_5c7e_4d8f_9f0c_c2b70683f5f0);

/// Connected headphones, reached by their Bluetooth address.
pub struct Headphones {
    pub name: String,
    pub address: String,
    bt_addr: u64,
}

impl Headphones {
    /// Finds the headphones among the devices connected to this PC: the one whose SDP records offer
    /// Sony's MDR channel.
    pub fn find() -> io::Result<Self> {
        start_winsock()?;
        if !has_radio()? {
            return Err(io::Error::other(
                "no Bluetooth adapter found\n\
                 If this PC has one, switch Bluetooth on in Settings > Bluetooth & devices.",
            ));
        }
        let mut found = Vec::new();
        let mut unasked = Vec::new();
        for device in connected_devices()? {
            match offers(device.bt_addr, MDR_UUID) {
                Ok(true) => found.push(device),
                Ok(false) => {}
                // A device that cannot be asked, such as one reached over Bluetooth LE only, is no
                // reason to give up on the others.
                Err(e) => unasked.push(format!("\n  {device}: {e}")),
            }
        }
        match found.len() {
            0 => {
                let unasked = if unasked.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\nThese connected devices could not be asked for their services:{}",
                        unasked.concat()
                    )
                };
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "no headphones found\n\
                         Switch Bluetooth on and connect the WH-1000XM4 to this PC, in Settings > \
                         Bluetooth & devices.{unasked}"
                    ),
                ))
            }
            1 => {
                let device = found.pop().unwrap();
                Ok(Headphones {
                    name: device.name,
                    address: address_string(device.bt_addr),
                    bt_addr: device.bt_addr,
                })
            }
            _ => {
                let names: Vec<String> = found.iter().map(|d| format!("\n  {d}")).collect();
                Err(io::Error::other(format!(
                    "several connected devices offer Sony's control channel:{}\n\
                     Disconnect all but the WH-1000XM4.",
                    names.concat()
                )))
            }
        }
    }

    /// Opens the channel with SDP UUID `uuid`, `label` naming it in errors and logs. With
    /// `verbose`, every byte sent and received is logged to stderr.
    pub fn open(&self, uuid: Uuid, label: &'static str, verbose: bool) -> io::Result<Channel> {
        let socket = Socket::connect(self.bt_addr, uuid).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "cannot open the {label} channel ({uuid}): {e}\n\
                     Make sure the headphones are still connected to this PC, and try again."
                ),
            )
        })?;
        Ok(Channel {
            label,
            verbose,
            socket,
        })
    }
}

/// Starts Winsock once; every socket and lookup call needs it started.
fn start_winsock() -> io::Result<()> {
    static STARTED: OnceLock<i32> = OnceLock::new();
    let code = *STARTED.get_or_init(|| {
        let mut data = WSADATA::default();
        // SAFETY: `data` is a WSADATA for the call to fill in. Winsock 2.2.
        unsafe { WSAStartup(0x0202, &mut data) }
    });
    if code != 0 {
        let e = io::Error::from_raw_os_error(code);
        return Err(io::Error::new(
            e.kind(),
            format!("cannot start Winsock: {e}"),
        ));
    }
    Ok(())
}

/// Whether this PC has a Bluetooth adapter.
fn has_radio() -> io::Result<bool> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut radio: HANDLE = ptr::null_mut();
    // SAFETY: `params` is filled in, and `radio` is a handle for the call to set.
    let find = unsafe { BluetoothFindFirstRadio(&params, &mut radio) };
    if find.is_null() {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
            Ok(false)
        } else {
            Err(io::Error::new(
                e.kind(),
                format!("cannot look for a Bluetooth adapter: {e}"),
            ))
        };
    }
    // SAFETY: both were handed out by the call above and are closed once.
    unsafe {
        CloseHandle(radio);
        BluetoothFindRadioClose(find);
    }
    Ok(true)
}

/// A Bluetooth device, as Windows lists it.
struct Device {
    name: String,
    bt_addr: u64,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, address_string(self.bt_addr))
    }
}

/// The devices connected to this PC, over any of its adapters.
fn connected_devices() -> io::Result<Vec<Device>> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnConnected: 1,
        ..Default::default()
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `params` and `info` are sized as the call expects.
    let find = unsafe { BluetoothFindFirstDevice(&params, &mut info) };
    if find.is_null() {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
            Ok(Vec::new())
        } else {
            Err(io::Error::new(
                e.kind(),
                format!("cannot list the connected Bluetooth devices: {e}"),
            ))
        };
    }
    let mut devices = Vec::new();
    loop {
        // SAFETY: the union holds the address in either form; the call filled it in.
        let bt_addr = unsafe { info.Address.Anonymous.ullLong };
        let name = info.szName.iter().position(|&c| c == 0).map_or_else(
            || String::from_utf16_lossy(&info.szName),
            |end| String::from_utf16_lossy(&info.szName[..end]),
        );
        devices.push(Device { name, bt_addr });
        // SAFETY: `find` is open, and `info` is sized as the call expects.
        if unsafe { BluetoothFindNextDevice(find, &mut info) } != 0 {
            continue;
        }
        let e = io::Error::last_os_error();
        // SAFETY: `find` is open and closed once.
        unsafe { BluetoothFindDeviceClose(find) };
        return if e.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
            Ok(devices)
        } else {
            Err(io::Error::new(
                e.kind(),
                format!("cannot list the connected Bluetooth devices: {e}"),
            ))
        };
    }
}

/// Whether the device at `bt_addr` offers a service with SDP UUID `uuid`, according to Winsock's
/// cache of the device's SDP records, which pairing filled. Nothing is sent to the device, as with
/// BlueZ's cached UUIDs on Linux; `LUP_FLUSHCACHE` would ask the device instead.
fn offers(bt_addr: u64, uuid: Uuid) -> io::Result<bool> {
    let mut class = guid(uuid);
    // The device, written the way Winsock writes a Bluetooth address.
    let mut context: Vec<u16> = format!("({})\0", address_string(bt_addr))
        .encode_utf16()
        .collect();
    let query = WSAQUERYSETW {
        dwSize: size_of::<WSAQUERYSETW>() as u32,
        dwNameSpace: NS_BTH,
        lpServiceClassId: &mut class,
        lpszContext: context.as_mut_ptr(),
        ..Default::default()
    };
    let flags = LUP_RETURN_ADDR;
    let mut lookup: HANDLE = ptr::null_mut();
    // SAFETY: `query` points at `class` and `context`, which outlive the lookup.
    if unsafe { WSALookupServiceBeginW(&query, flags, &mut lookup) } != 0 {
        return no_such_service(wsa_error());
    }
    // A result is a WSAQUERYSETW followed by what it points at; 4 KB holds any one record.
    let mut buffer = [0u64; 512];
    let mut len = size_of_val(&buffer) as u32;
    // SAFETY: `buffer` is `len` bytes, aligned for the WSAQUERYSETW written at its start.
    let result =
        unsafe { WSALookupServiceNextW(lookup, flags, &mut len, buffer.as_mut_ptr().cast()) };
    let error = wsa_error();
    // SAFETY: `lookup` was begun above and is ended once.
    unsafe { WSALookupServiceEnd(lookup) };
    if result == 0 {
        Ok(true)
    } else {
        no_such_service(error)
    }
}

/// `Ok(false)` for the errors that mean the device offers no such service; any other is returned.
fn no_such_service(e: io::Error) -> io::Result<bool> {
    match e.raw_os_error() {
        Some(WSASERVICE_NOT_FOUND | WSA_E_NO_MORE | WSAENOMORE) => Ok(false),
        _ => Err(e),
    }
}

/// A Bluetooth address as it is usually written, like `00:11:22:33:44:55`.
fn address_string(bt_addr: u64) -> String {
    bt_addr.to_be_bytes()[2..]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// `uuid` as the GUID Winsock takes.
fn guid(uuid: Uuid) -> GUID {
    GUID::from_u128(uuid.0)
}

/// The error of the last Winsock call.
fn wsa_error() -> io::Error {
    // SAFETY: no preconditions.
    io::Error::from_raw_os_error(unsafe { WSAGetLastError() })
}

/// The error of the last Winsock call on an open channel. Every code that means the link to the
/// headphones is gone comes back as [`io::ErrorKind::ConnectionReset`], whichever the stack chose:
/// after the commit, the upload takes a lost link as the headphones restarting.
fn link_error() -> io::Error {
    let e = wsa_error();
    match e.raw_os_error() {
        Some(
            WSAENETDOWN | WSAENETUNREACH | WSAENETRESET | WSAESHUTDOWN | WSAEHOSTDOWN
            | WSAEHOSTUNREACH | WSAEDISCON,
        ) => io::Error::new(io::ErrorKind::ConnectionReset, e),
        _ => e,
    }
}

/// A connected Bluetooth socket, closed on drop.
struct Socket(SOCKET);

impl Socket {
    /// Connects to the service with SDP UUID `uuid` on the device at `bt_addr`. With no channel
    /// number given, Windows looks it up over SDP.
    fn connect(bt_addr: u64, uuid: Uuid) -> io::Result<Self> {
        start_winsock()?;
        // SAFETY: Winsock is started; no other preconditions.
        let raw = unsafe { socket(AF_BTH.into(), SOCK_STREAM, BTHPROTO_RFCOMM as i32) };
        if raw == INVALID_SOCKET {
            return Err(wsa_error());
        }
        let socket = Socket(raw);
        let address = SOCKADDR_BTH {
            addressFamily: AF_BTH,
            btAddr: bt_addr,
            serviceClassId: guid(uuid),
            port: 0,
        };
        // SAFETY: `address` is a SOCKADDR_BTH, passed with its length.
        let connected = unsafe {
            connect(
                raw,
                (&raw const address).cast::<SOCKADDR>(),
                size_of::<SOCKADDR_BTH>() as i32,
            )
        };
        if connected != 0 {
            return Err(wsa_error());
        }
        Ok(socket)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        // SAFETY: the socket is open and closed once.
        unsafe { closesocket(self.0) };
    }
}

/// One RFCOMM channel to the headphones.
pub struct Channel {
    label: &'static str,
    verbose: bool,
    socket: Socket,
}

impl Transport for Channel {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.verbose {
            eprintln!("{} > {}", self.label, hex(bytes));
        }
        let mut rest = bytes;
        while !rest.is_empty() {
            // SAFETY: `rest` is readable for its length.
            let sent = unsafe { send(self.socket.0, rest.as_ptr(), rest.len() as i32, 0) };
            if sent == SOCKET_ERROR {
                return Err(link_error());
            }
            if sent <= 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!("nothing could be sent on the {} channel", self.label),
                ));
            }
            rest = &rest[sent as usize..];
        }
        Ok(())
    }

    fn receive(&mut self, wait: Duration) -> io::Result<Vec<u8>> {
        let mut readable = FD_SET {
            fd_count: 1,
            ..Default::default()
        };
        readable.fd_array[0] = self.socket.0;
        let timeout = TIMEVAL {
            tv_sec: wait.as_secs().min(i32::MAX as u64) as i32,
            tv_usec: wait.subsec_micros() as i32,
        };
        // SAFETY: `readable` holds one open socket; the other sets are absent.
        let ready = unsafe { select(0, &mut readable, ptr::null_mut(), ptr::null_mut(), &timeout) };
        if ready == SOCKET_ERROR {
            return Err(link_error());
        }
        if ready == 0 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out"));
        }
        let mut buffer = vec![0; 1024];
        // SAFETY: `buffer` is writable for its length.
        let n = unsafe { recv(self.socket.0, buffer.as_mut_ptr(), buffer.len() as i32, 0) };
        if n == SOCKET_ERROR {
            return Err(link_error());
        }
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("the headphones closed the {} channel", self.label),
            ));
        }
        buffer.truncate(n as usize);
        if self.verbose {
            eprintln!("{} < {}", self.label, hex(&buffer));
        }
        Ok(buffer)
    }
}
