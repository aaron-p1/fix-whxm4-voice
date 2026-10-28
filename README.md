# fix-whxm4-voice

This readme was written by a human. All other files were vibe coded.

This tool allows modifying the sound clips of the Sony WH-1000XM4 headphones (tested on firmware `3.0.1`).
It was created mainly to stop them from chattering over every little thing
while I am trying to listen to content and to make the actions faster by not waiting
for the messages to finish.

It works by building a custom voice pack and uploading it to the headphones.

This project is based on reverse engineering. The risk of bricking should be very low,
but I cannot guarantee anything. See [SAFETY.md](SAFETY.md) for more information.
(TL;DR: the voice pack is first uploaded to a staging area and the headphones verify and install it themselves.
No firmware is modified, so I don't really see a reason for them to brick.)

## Usage

### Requirements

- Linux
- [nix package manager](https://nixos.org) with flakes enabled
  (or just build it as a normal rust program, which needs `ffmpeg` and `dbus` dev headers)
- [bluez bluetooth service running](https://wiki.archlinux.org/title/bluetooth)
  (probably already running if bluetooth works on your system)

On macOS instead (see [ADR 0006](docs/adr/0006-macos-through-an-iobluetooth-helper.md)):

- the Xcode command line tools (`xcode-select --install`), which build the small Swift Bluetooth helper
- Rust and `ffmpeg` (e.g. `brew install rustup ffmpeg`)
- build with `cargo build --release` and run `target/release/fix-whxm4-voice` in place of the `nix shell` step
- the first command that talks to the headphones makes macOS ask for Bluetooth access for `xm4-rfcomm`

On Windows instead (see [ADR 0007](docs/adr/0007-reach-the-headphones-through-winsock-on-windows.md)):

- a Rust toolchain from [rustup](https://rustup.rs): the MSVC one, which needs the Visual Studio C++ build
  tools it points you to
- `ffmpeg` on the `PATH` (e.g. `winget install Gyan.FFmpeg` or `choco install ffmpeg`)
- [Git for Windows](https://gitforwindows.org): its bash runs the sound scripts of `generate`
- the headphones paired and connected in Settings > Bluetooth & devices
- build and install with `cargo install --git https://github.com/aaron-p1/fix-whxm4-voice` in place of the
  `nix shell` step, and run `fix-whxm4-voice` from PowerShell or cmd

### Steps

Get the tool with:

```bash
nix shell github:aaron-p1/fix-whxm4-voice
```

Run the tool in an empty directory:

```bash
fix-whxm4-voice
```

(This creates dirs `original`, `changes`, `build`, `stock`
and downloads the original English voice pack into `stock` and extracts it into `original`.)

Now audio files (any format) or empty files can be added into `changes` to replace the original ones.
Each file must start with the number of the original file it replaces.
The files in `original` should not be modified.
Empty files will be replaced with a short silent clip when the pack is built.

It is also possible to use the default sound theme defined in this repo by running:

```bash
fix-whxm4-voice generate
```

(This will run the scripts in `sounds/` to generate beeps
and optionally extract numbers from the English battery level messages.)

The scripts can be run standalone as well. So if you want to modify them,
just download and change them.

After changes were made, they can be uploaded (the headphones must be connected to the PC):

```bash
fix-whxm4-voice
```

(It will convert audio files, check their size, build the voice pack
and upload it to the headphones after asking for confirmation.
Don't open the Sony app during the upload. It's unknown whether doing so would break anything.
By default it will install the custom voice pack as `German`
because English cannot be replaced.)

After the progress bar is done, the headphones will automatically "power off",
install the voice pack and power back on.

The command has help pages, subcommands and options for each step.
(For example `--lang` for selecting the language to download)

### Reverting to stock

Upload originals without any changes (but it would install the English voice pack as German)
or just select another language in the Sony app (untested).

## References

Projects that were useful for reverse engineering.

(Only the LLM looked at these projects and this section is mostly written by an LLM)

- [SantiMoncada/sony-xm4-voice-mod](https://github.com/SantiMoncada/sony-xm4-voice-mod)
  - A working end-to-end mod: pack builder, manifest forging, and a mitmproxy addon that serves the pack to the
    Sony app.
  - Its `NOTES.md` describes the pack format. It also found that modding English on an English device fails.
- [HelgeSverre/sony-vp-extract](https://github.com/HelgeSverre/sony-vp-extract)
  - A voice-pack extractor. Its `docs/WRITEUP.md` explains the format and how the key is derived.
- [auracast-research/race-toolkit](https://github.com/auracast-research/race-toolkit)
  - Airoha RACE protocol tooling, used to cross-check the RACE/FOTA commands, header layout and page checksum.
  - The repo has no licence file, so it was used only as a reference and no code was taken from it.
- [Freeyourgadget/Gadgetbridge](https://codeberg.org/Freeyourgadget/Gadgetbridge)
  - The source of the MDR device-info queries (model name, firmware version) that `device-info` sends.

## Disclaimer

This project is not affiliated with, endorsed by or supported by Sony. "Sony" and "WH-1000XM4" are
trademarks of their respective owners.

Using this tool modifies data on your headphones in a way Sony does not support. It may void your
warranty and could, in the worst case, leave the headphones unusable. You use it entirely at your
own risk; the authors accept no liability for any damage.

The tool downloads Sony's voice packs from Sony's public update server. No Sony audio is
distributed with this project.

## License

[MIT](LICENSE)
