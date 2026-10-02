# touchHLE core: plan and design log

Requested in chimera#177 (2026-10-01); feasibility evaluated and the project
started 2026-10-02. Upstream: touchHLE 0.3.0 (8eb3418), MPL-2.0, Rust, with
dynarmic (0BSD) for the ARM CPU.

## Why touchHLE fits a TAS core

- **One host thread.** Every app thread is a corosensei coroutine on the one
  thread touchHLE runs on, switched every 100,000 dynarmic ticks
  (`Environment::run`/`schedule_next_thread`). Scheduling is deterministic by
  construction; there is no thread pool to tame.
- **No fastmem.** dynarmic runs in page-table mode over a 4 GiB block of
  memory touchHLE owns, so no fault handler is involved. The Azahar core
  already runs dynarmic in the sandbox.
- **No firmware.** touchHLE IS the operating system (HLE). The five iPhone OS
  libraries apps link against are free software (`touchHLE_dylibs/README.md`),
  the fonts are Liberation and Noto. The core carries them all; a project is
  only the app.

## Decisions

### The picture: an OpenGL compiled into the core (2026-10-02)

touchHLE draws an app's OpenGL ES 1.1 through its GLES1-on-GL2 layer, which
needs a desktop OpenGL 2.1 COMPATIBILITY context: fixed function, matrix
stacks, client arrays. The GPU bridge's 760 entry points are core-profile
only (no glMatrixMode, glVertexPointer...), so the core draws with Mesa
softpipe behind OSMesa, built into it (`waterbox/setup-mesa.sh`, the release
and options pcsx2/flycast pin), whose legacy context is 3.3 compatibility.

The native reference uses a HOST build of the same Mesa (`setup-mesa.sh -n`),
never the host's own OpenGL: an app can query its OpenGL (glGetString,
glGetIntegerv, glReadPixels) and act on the answer, so the OpenGL is part of
the machine. A GPU path would mean adding ~100 fixed-function entry points to
the bridge and a compatibility context on the host; not planned.

### The clock: bought with instructions (2026-10-02)

The sandbox clock is frozen, and touchHLE reads the host clock in ~25 places
(scheduler sleeps, NSTimer, the EAGL frame limiter, NSDate, CFAbsoluteTime,
`time()`, `mach_absolute_time()`, audio units). Patch 0001 routes them all
through `crate::time` (`src/chimera/time.rs`), which keeps std's names:

- machine time = executed instructions at the CPU clock (412 MHz, the first
  iPhone's and the 3G's ARM11) + time skipped while every thread slept;
- when every thread is asleep the scheduler jumps the clock to the next
  deadline instead of sleeping;
- the date starts at the `startDate` setting (2000-01-01) and moves with it.

Upstream's own build keeps std's clocks (`crate::time` re-exports them).

### Frames: touchHLE's main loop in a coroutine (2026-10-02)

`Environment::run` never returns. The core runs touchHLE's `main` inside a
corosensei coroutine of its own (`src/chimera/mod.rs`) that yields at the end
of every frame. A frame is 1/60 s of machine time and ends:

- between two threads' turns, once the clock has passed the frame's end
  (`frame_boundary`, called where `run` polls the window); or
- when every thread is asleep past the frame's end: the clock moves to the end
  exactly, not past it (`idle_until`).

The next frame's input is polled the moment the machine resumes. Where a frame
ends depends only on the machine, never on the host.

### No SDL (2026-10-02)

`sdl2` is optional behind a new `sdl` feature; the `chimera` feature swaps
`window.rs` for `src/chimera/window.rs`, which keeps its public shape:

- the screen is a memory buffer OSMesa draws into; `swap_window` copies the
  picture out (BGRA, top row first) for the frontend;
- input is levels set once per frame (`set_touch`, `set_tilt`);
  `poll_for_events` turns the change into touch events ONE FINGER PER EVENT,
  in finger order - upstream's events carry a HashMap, whose iteration order
  is seeded per process;
- locale (en/US), battery (full), URL opening (refused) and message boxes
  (stderr) are fixed answers: they are part of the machine;
- resources (dylibs, fonts, default options) are built into the core
  (`src/chimera/resources.rs`), served through upstream's `ResourceFile`;
- touchHLE's log file is not written; its output goes to stderr only.

### Input (2026-10-02)

- `Touch 1`, `Touch 2`: a finger on the screen (buttons).
- `Touch 1 X/Y`, `Touch 2 X/Y`: where, 0..65535 across and down the picture as
  shown, neutral 32768, as every absolute position in Chimera is. The core
  converts with the live picture size (`pixel = axis * size / 65536`), so a
  rotated app is touched where it is seen.
- `Tilt X`, `Tilt Y`: -32768..32767, what an analog stick does in upstream's
  accelerometer simulation (and its tilt range/offset options still apply).

## Milestones

- [x] M0 native: stock touchHLE builds; TestApp built from upstream's source
  (`waterbox/build-testapp.py`, clang + touchHLE's common-3.0 SDK) passes its
  105 CLI tests on stock touchHLE.
- [x] M1 native reference: the `chimera` feature builds; TestApp's UIKit
  screen draws through the host Mesa; machine time is exactly 1 s per 60
  frames; two runs are byte-identical; a tap navigates.
- [x] M2 guest build (`waterbox/build-guest.sh`): cargo -Z build-std for
  `guest/waterbox-guest.json` (Ruffle's target: no red zone, no native TLS),
  the cc/cmake crates handed the guest kit's compilers through their
  `CC_waterbox_guest`/`CMAKE_TOOLCHAIN_FILE_waterbox_guest` variables, thread
  locals defined away; linked with the guest Mesa and the C++ kit's libstdc++
  and libgcc_eh. check-wbx clean. `waterbox/run-wbx.c` runs it as the engine
  does and prints run-native's lines. TestApp: native and sandbox digests
  identical over 300 frames; a tap, a savestate round trip before every frame
  (85 MB a state) and a state moved to a fresh host all end on the same
  picture; the CLI suite passes 105/105 in both, output identical.
- [x] M3 filesystem (patch 0003): Documents, Library/{Preferences,Caches} and
  tmp exist only in memory (`FileLocation::Memory`), directories list in name
  order (a BTreeMap in this build). Save-data export/import is still to do.
- [x] M4 sound (patch 0005): every device touchHLE opens - the app's own
  alcOpenDevice and touchHLE's for AudioQueue and system sounds - is an
  OpenAL Soft loopback device; at the end of each frame the core renders 735
  stereo pairs (44.1 kHz / 60) from each and mixes them. OpenAL Soft's output
  backends are not built at all (its null backend mixes on a thread that
  sleeps on the host clock), only its SSE2 mixer is (patch 0002). Its event
  thread still starts per context: miniBox runs it as a green thread that
  only wakes when a context is torn down. `waterbox/tests/apps/SoundTest` (our
  own app: silence for 1 s, one OpenAL tone, then two) sounds the same in
  both flavors, through a savestate before every frame and a state moved to
  a fresh host (hash of every frame's sound identical).
- [ ] M5 savestates: the rerecord and session runs work (above); make them
  gate legs on real games.
- [ ] M6 package, gate, settings; games from the user.

### An app that exits (2026-10-02)

Upstream ends the process when an app calls `exit()` or UIKit terminates it;
in a sandbox that is the guest dying. Patch 0004 instead stops the machine
where it stands (`chimera::exit_machine`): the core reports it is not running
and keeps the last picture.

### The patch series

Patches are written with `waterbox/make-patch.py`, never with a bare
`git diff`: the baseline is HEAD plus the patches before, so a file two
patches touch does not end up in both. `--amend NNNN` rewrites a patch from
the tree (its files only).

## Open questions

- Rust's HashMap seeds come from getrandom: a fixed stream in the sandbox,
  fresh randomness natively. Any HashMap order that reaches the app would
  make native and sandbox differ (the gate would show it). Touch events and
  directory listings are already ordered.
- OpenAL Soft runs a mixer thread with a real backend; the loopback device
  should avoid it. Check its event thread.
- Rust panics: the Ruffle core builds the guest `panic = "abort"`; touchHLE's
  `catch_unwind` paths are only for crash reports, so the same should do.
