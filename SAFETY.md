# Is it safe to upload a voice pack?

This page explains what `upload` does to your WH-1000XM4, what state the headphones are left in if
something goes wrong, and how to get them back to normal. Read it before your first upload.

**Status (2026-10-28):** two uploads have succeeded on real headphones: one from Linux, while they
were also connected to a phone, and one from Windows (ADR 0007). None of the failures below has been
tried on real headphones. What this page says about them comes from how the Sony app does the same
job and from tests against simulated headphones. Where something is not known, this page says so.

## Short answer

- **Bricking risk: low, but not zero, and not proven.** The tool uses the same mechanism the Sony
  app uses for voice packs. It never touches the headphones' firmware. The pack is checked twice:
  by the tool before sending, and by the headphones before they accept it.
- **The likely worst case is wrong or missing voice prompts,** not headphones that won't turn on.
  Music, calls, noise canceling and pairing don't depend on the voice pack.
- **The usual fix is the Sony app:** pick a stock voice-guidance language there, and the app
  installs Sony's own voice pack again. See
  [Getting back to Sony's voice prompts](#getting-back-to-sonys-voice-prompts).
- **English is always there.** English is built into the headphones and is probably never replaced
  by an upload. The tool warns and asks before building a pack as English.

## What the upload writes, and what it never writes

The headphones have a storage area for incoming updates. They report where it is when asked. The
upload:

- writes **only** inside that area, and only as many bytes as the voice pack needs (rounded up to
  4 KB). A built-in guard checks every command before it is sent and refuses anything outside that
  range, and any command that isn't part of a voice-pack update.
- **never** sends firmware, and never writes outside the area the headphones reported.
- erases or writes **nothing until you type `upload`** after seeing the plan. Run `upload --dry-run`
  to see the plan without any risk.
- only asks the headphones to install the pack after **they** have checked it and said it is
  intact. A pack that fails their check is never installed.

The pack itself is a normal Sony voice pack with your clips in it. Before sending, the tool unpacks
it again and checks every checksum, the clip table and every clip.

**A known uncertainty:** the area the headphones reported on our test unit (at `0x510000`, about
6 MB) seems to be the same area where they keep the installed voice pack. If that is right, an
upload that breaks off in the middle can leave the installed voice pack incomplete. That would
affect the prompts in that language, nothing else, and the fixes below still apply.

## Before you start

- Charge the headphones. A dead battery in the middle of an upload is one of the failures below.
- Keep the headphones close to the PC, and don't turn them off during the upload.
- Your phone can stay connected to the headphones. During the upload, leave the headphones alone:
  don't play audio on any device, don't press their buttons, and don't open the Sony app. The tool
  reminds you before you confirm.
- Make sure the Sony app is installed on your phone, or that you can install it. It is how you get
  back to Sony's voice prompts.

## What can go wrong, and what state the headphones are left in

An upload goes through four stages. For each one, this section says what can fail, what state the
headphones are left in, and what to do.

At the start, the tool switches on a mode the headphones need before they accept an upload (shown
as "Language update mode" by `device-info`). The tool switches it off again at the end, and after
an error if the connection still works. See [Update mode left on](#update-mode-left-on).

### Stage 1: preparing (nothing is written yet)

**What can fail:** the build fails (bad change name, clips too large, ffmpeg error), no headphones
are found, a connection or query fails, or you don't type `upload` when asked.

**State of the headphones:** unchanged.

**What to do:** nothing. Fix the cause and run `upload` again.

### Stage 2: erasing and writing (the progress bar is moving)

**What can fail:** the connection drops, the headphones turn off or go out of range, they stop
answering for 5 seconds, or they report an error or cancel the upload.

**Stopping it yourself:** press Ctrl-C. The tool finishes the command in flight, sends nothing more,
switches update mode off and says so. Nothing is installed, and the next `upload` continues where it
stopped. Ctrl-C up to the checksum check leaves the headphones as below; Ctrl-C after it leaves them
as in stage 4's first case. A second Ctrl-C quits at once and may leave update mode on.

**State of the headphones:**

- The update area is half-written. Nothing is installed.
- The prompts they had should still play. The exception is the uncertainty above: if the update area
  holds the installed voice pack, the prompts in that language may be wrong or missing until you
  finish an upload or restore Sony's pack.
- Update mode may still be on.

**What to do:** make sure the headphones are on and connected to the PC, and run `upload` again. It
continues where it stopped: parts already written are kept. Or restore Sony's prompts with the Sony
app. You don't need to restart the headphones first (see [Is a restart needed?](#is-a-restart-needed)).

### Stage 3: the headphones check the pack (the bar stops at about 99 %)

**What can fail:** the headphones reject the pack. The tool says "checksum check failed, so they
rejected the pack".

**State of the headphones:** as in stage 2. The pack is written but rejected, and nothing is
installed.

**What to do:** run `upload` again. If it is rejected again, please report it as a bug: the pack
the tool built doesn't match what the headphones expect. You can restore Sony's prompts with the
Sony app.

### Stage 4: the install command

After a passed check, the tool sends the install command. The headphones then restart and install
the pack themselves, just as after an update from the Sony app. The tool counts the dropped
connection as success and prints "done".

**What can fail, and what state each failure leaves:**

- **The connection drops before the install command, or the headphones refuse it.** The headphones
  hold your checked pack, ready to install, but haven't installed it. Run `upload` again (no restart
  needed). The tool compares the waiting pack with yours. If they match, it shows this in the
  plan and only sends the install command once you confirm.
- **The headphones neither answer nor restart within 15 seconds.** The tool reports this, but they
  may still install the pack. Wait for them to restart, then check with `device-info`.
- **They lose power while installing.** The result is the same as a power loss during a normal
  voice-pack update from the Sony app. This tool can't affect it: the headphones install on their
  own.

**After "done":** wait for the restart. Run `device-info`: the current language should be the install
language (German by default). Then listen to the prompts.

### A different pack is waiting to be installed

If the headphones hold a checked pack that isn't the one you are uploading (for example, you edited
your changes after a stage 4 failure), the plan says so, and the upload writes your pack over it like
any other content of the update area. The Sony app never does this (it would install the waiting
pack), so it is not known whether the headphones accept it. If they refuse, the tool stops with an
error and the headphones are left as in stage 2.

### Update mode left on

After a failure in stage 2 or 4, or a second Ctrl-C, update mode may still be on. While it is on,
they should still play music and take calls. Run `device-info` to see whether it is on. You don't
have to do anything about it: the next `upload` switches it on anyway, and the Sony app switches it
as it needs.

### Is a restart needed?

No step on this page requires turning the headphones off and on. The tool doesn't rely on it: every
`upload` switches update mode on and starts a new upload session by itself.

A restart is only worth trying if the headphones refuse the next `upload`, or the Sony app's update,
after a failure. The reason: a failed upload can leave update mode on and an upload session open.
Turning the headphones off and on is expected to clear both, but this has not been checked.

## Getting back to Sony's voice prompts

1. Connect the headphones to your phone and open the Sony app.
2. In the Sony app, open the voice-guidance language setting and pick a stock language, such as
   English or the language you want. The app downloads Sony's voice pack and installs it, the same
   way as for a normal language change.

If the app says the update failed, try again, or pick another language first and then the one you
want. If a checked pack was waiting to be installed (stage 4), the first attempt installs that pack
instead, and a second attempt restores Sony's.

Resetting or initializing the headphones (see Sony's help guide for the WH-1000XM4) clears
settings and pairings. **It is not known whether it restores the voice prompts,** so use the Sony
app for that.

## Known limitations

- **No retries.** The Sony app resends a command that went unanswered, up to 3 times. This tool
  stops at the first command that goes unanswered for 5 seconds, which leaves the headphones as in
  stage 2. Running `upload` again continues from there.
- **The install isn't watched.** After "done", the tool doesn't wait for the headphones to come back
  or check that they installed the pack, and doesn't switch update mode off afterwards. The Sony app
  does both. Check with `device-info`.

## What isn't known yet

- Whether the area the upload writes to is the area holding the installed voice pack (see above).
- Whether turning the headphones off and on clears update mode and an open upload session.
- Whether the headphones accept an upload over a different pack that is waiting to be installed.
- Whether resetting or initializing the headphones restores Sony's voice prompts.

The technical background, with every command and what the headphones answered on our test unit, is
in [docs/upload-protocol.md](docs/upload-protocol.md).
