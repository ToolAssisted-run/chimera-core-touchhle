# chimera-core-touchhle

[touchHLE](https://github.com/touchHLE/touchHLE)'s iPhone OS 2.x-4.0 app
emulator as a [Chimera](https://github.com/ToolAssisted-run/chimera) core,
running in miniBox's sandbox. A project is one decrypted `.ipa`, nothing
else: touchHLE reimplements iPhone OS and the core carries the free libraries
and fonts apps load. [appdb.touchhle.org](https://appdb.touchhle.org/) says
which apps run.

- Two fingers on the screen (0..65535 on the picture as shown, so a sideways
  app is touched where it is seen) and the accelerometer as a tilt stick.
- Machine time is bought with the app's own instructions (412 MHz by default)
  and nothing reads the host clock; the date starts at a setting.
- The picture is drawn by Mesa's softpipe inside the core, the sound mixed by
  OpenAL Soft a frame at a time, and what the app saves lives in the machine:
  savestates carry it, Export Save Data takes it out, a save-data slot puts it
  back.

Layout: `extern/touchHLE` (upstream, pinned; clone with `--recursive`),
`patches/` (the series that makes it a frame-at-a-time machine),
`waterbox/` (the core, the native reference, the builds and the gate) and
`docs/PLAN.md` (the design and the reasoning behind every decision).

The gate runs on touchHLE's own TestApp and this repository's test apps, all
built from source, and on any decrypted `.ipa` in `tests/roms-local/`
(`waterbox/tests/game-list.txt`). Report problems in
[chimera's issues](https://github.com/ToolAssisted-run/chimera/issues).

## Using it in Chimera

Chimera ships no cores and downloads nothing. Download the `.chimeraCore`
package from this repository's
[Releases](https://github.com/ToolAssisted-run/chimera-core-touchhle/releases)
page, or build it, and put it in the `Cores` folder beside `Chimera.exe` (or
the folder chosen in File > Core Manager > Change folder...). File > Core
Manager lists the cores in that folder. The same package works on Linux and on
Windows.

## Building

`waterbox/build-package.sh -m <miniBox dir> -r <chimera checkout>` builds the
guest Mesa and the core and writes
`<chimera checkout>/build/Cores/touchhle.chimeraCore`. The requirements, the
steps that come before it and the gate are in
[docs/BUILDING.md](docs/BUILDING.md). [AGENTS.md](AGENTS.md) is the short
operating guide for a coding agent.
