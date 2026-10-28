# Reach the headphones through Winsock's Bluetooth sockets on Windows, letting Windows find the channels

Windows has no BlueZ, but Winsock has Bluetooth sockets (`AF_BTH`, `BTHPROTO_RFCOMM`). A socket
connected to a device address and a service class UUID, with the channel number left at 0, makes
Windows look the channel up over SDP and connect. So the channel numbers stay unknown to the tool,
as on Linux (ADR 0004). The headphones are found among the devices Windows reports as connected
(`BluetoothFindFirstDevice`), as the one whose SDP records offer Sony's MDR channel. The records are
read from the cache Windows filled when pairing, through `WSALookupServiceBegin` without
`LUP_FLUSHCACHE`, so no device is asked anything, as with BlueZ's cached UUIDs on Linux.

The Windows backend, `src/bluetooth_windows.rs`, is raw `windows-sys` calls behind the same
`Headphones` and `Channel` as the Linux backend, so `main` does not change. `src/lib.rs` picks the
backend per OS, as it does for macOS (ADR 0006), and on any other OS a stub,
`src/bluetooth_unsupported.rs`, says Bluetooth is unsupported there while download, generate and
build keep working. `windows-sys` is a Windows-only dependency. The Windows sockets block, and a reply
is awaited with `select`, so that a timeout leaves the socket untouched: Winsock says a socket whose
receive timed out through `SO_RCVTIMEO` should be closed.

The alternative was the WinRT `RfcommDeviceService` API, which is asynchronous, pulls in the much
larger `windows` crate and wants its own device enumeration. Winsock's socket API is small, stable,
and does the SDP lookup itself.

`generate` keeps its bash scripts and runs them with Git for Windows' bash. The `bash` on a Windows
PATH is usually WSL's, which cannot open the Windows paths the tool hands it, such as the script and
the staging folder. So the tool finds Git's `bin\bash.exe` through `git` on the PATH or in Git's
default install folders, unless `FIX_WHXM4_VOICE_BASH` names another bash. Git's bash brings awk,
mktemp and the other tools the scripts use, and converts the paths it hands to ffmpeg. Porting the
scripts to PowerShell was rejected: the theme and the cut points would exist twice. ffmpeg must be
on the PATH, as on Linux outside Nix.

Verified on Windows 11 with a WH-1000XM4 on firmware 3.0.1: `device-info` finds the headphones
among the connected devices, opens both channels and gets every reply; `download`,
`generate --battery` with both scripts, `build` and `upload --dry-run` run through, the dry run
switching language update mode on and off and reporting the same FOTA partition as on Linux. A real
upload then wrote a 477 KB pack in 1850 pages in about a minute, the headphones passed their check
and installed it as German.
