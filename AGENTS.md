# AGENTS.md - touchHLE core for Chimera

This repository builds touchHLE (iPhone OS 2.x to 4.0 apps) as a sandboxed
guest for [Chimera](https://github.com/ToolAssisted-run/chimera), a frontend
for tool-assisted speedruns, and packs it into one file,
`touchhle.chimeraCore`. Upstream touchHLE is a pinned submodule; everything
here is the patch series that makes it a frame-at-a-time machine, the adapter,
the build scripts and the gate. The core is Rust with C and C++ dependencies.
Builds run on Linux, and the one package works on Linux and on Windows.

## Layout

- `extern/touchHLE` - upstream touchHLE, a submodule pinned by commit, with
  submodules of its own. `extern/ext-boost` - the Boost headers dynarmic needs.
- `patches/` - the numbered patch series applied to `extern/touchHLE` by
  `waterbox/apply-patches.sh`, which refuses a partly patched tree.
  `waterbox/make-patch.py` writes or amends a patch from the working tree.
- `waterbox/guest/` - the core's crate (`src/lib.rs` holds the exports the
  engine calls, `src/mathlib.rs` the maths), its `.cargo/config.toml` and its
  target, `waterbox-guest.json`. Built twice: for the guest and for the host.
- `waterbox/run-native/` - the native reference: the same crate on the host.
  `waterbox/run-wbx.c` - the runner that loads `core.wbx` through miniBox.
- `waterbox/guest-syscalls.cpp`, `guest-toolchain.cmake` - guest shims and the
  CMake toolchain file for the C and C++ dependencies.
- `waterbox/setup-mesa.sh`, `build-guest.sh`, `build-package.sh` - the builds.
- `waterbox/run-gate.sh` - the gate. `waterbox/build-testapp.py` - builds the
  gate's test apps. `waterbox/tests/apps/` - this repository's own test apps.
  `waterbox/tests/game-list.txt` - the game legs.
- `waterbox/waterbox.config`, `file_slots.json`, `default_keybinds.json`,
  `package-licenses.json` - what the package declares.
- `docs/PLAN.md` - the design log. `.github/workflows/chimera.yml` - CI, the
  authoritative build recipe.
- `build/` - all build output. `tests/roms-local/` - the user's own `.ipa`
  files. Both are ignored by git.

## Set up the build environment

`<chimera>` is a Chimera checkout and `<minibox>` is
`<chimera>/extern/chimera-common-minibox`.

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends meson ninja-build build-essential cmake pkg-config python3 bison flex python3-mako python3-packaging clang ffmpeg mono-complete libgl1-mesa-dev libx11-dev libxext-dev libasound2-dev
rustup toolchain install nightly --profile minimal -c rust-src
rustup default nightly
git submodule update --init --recursive
git clone https://github.com/ToolAssisted-run/chimera <chimera>
git -C <chimera> submodule update --init --recursive
( cd <chimera>
  meson setup build/meson-linux --prefix "$PWD/build" --libdir dll
  meson compile -C build/meson-linux
  meson install -C build/meson-linux
  dotnet build source/gui/Chimera.sln -c Release /nodeReuse:false -p:UseSharedCompilation=false )
mb=<minibox>
[ -f "$mb/build/meson-linux/build.ninja" ] || meson setup "$mb/build/meson-linux" "$mb"
meson compile -C "$mb/build/meson-linux"
[ -f "$mb/build/meson-cpp/build.ninja" ] || meson setup "$mb/build/meson-cpp" "$mb" -Dguest_cpp=true
meson compile -C "$mb/build/meson-cpp"
```

Building Chimera (the block in parentheses) needs the .NET SDK 8.0. Only the
gate's engine leg and the contract tests need Chimera built.

## Build

```sh
./waterbox/build-package.sh -m <minibox> -r <chimera>
```

That one command fetches and builds the guest Mesa (`setup-mesa.sh -m`),
applies the patches, builds the core (`build-guest.sh -m`, which runs
`cargo +nightly build --release` in `waterbox/guest`) and writes
`<chimera>/build/Cores/touchhle.chimeraCore`. The unstripped core is
`build/wbx/core.wbx`.

Pass `-m` and `-r`: `build-guest.sh` and `run-gate.sh` stop without `-m` (or
`MINIBOX_DIR`), and `build-package.sh` guesses the Chimera checkout without
`-r`. The native reference has no build script: `run-gate.sh` builds it.

## Install the core into Chimera

Chimera ships no cores and downloads nothing: a package is put in its `Cores`
folder by hand. In a source checkout `build-package.sh -r <chimera>` already
wrote it there, as `<chimera>/build/Cores/touchhle.chimeraCore`. For a release
bundle, copy the file into the `Cores` folder beside `Chimera.exe`, or into
the folder chosen in File > Core Manager > Change folder... File > Core
Manager lists the folder; Refresh List rescans it.

A hand-built package is stamped `<commit>+local` (`-dirty` when the tree has
changes) and is for testing. CI stamps the commit and publishes the `dev` and
`nightly-YYYY-MM-DD` releases.

## Test before you commit

The gate applies the patches and builds what it runs:

```sh
./waterbox/run-gate.sh -m <minibox> -r <chimera>
# also, when the package or its declarations changed:
cd <chimera> && CHIMERA_CORES_DIR=<chimera>/build/Cores dotnet test source/gui/Chimera.Tests.Client.Common/Chimera.Tests.Client.Common.csproj -c Release --nologo --filter "FullyQualifiedName~InstalledCorePackagesTests|FullyQualifiedName~MnemonicUniquenessTests"
```

`run-gate.sh` builds what is missing (host Mesa, native reference, core,
`run-wbx`, the SDK it downloads, the test apps) and runs every leg. `-n` skips
the rebuild. The last line must read `gate: N passed, 0 failed, ...` and the
script must exit 0. Logs are in `build/gate/`.

Expected SKIPs: `games` when `tests/roms-local/` holds no `.ipa`, and
`the engine` when `-r` does not name a checkout with `chimera-run` and the
package. Give `-r` so it runs. Never report a skipped leg as passed.

## Rules of this repository

- Never commit inside `extern/touchHLE`. A change to upstream is a numbered
  file in `patches/`, applied at the start of every guest build. `git status`
  shows `extern/touchHLE` as modified once the series is applied. That is
  normal: do not stage it, reset it or clean it to tidy up.
- Write patches with `waterbox/make-patch.py`, never with a bare `git diff`:
  `python3 waterbox/make-patch.py --check`, then
  `python3 waterbox/make-patch.py <NNNN-chimera-what-is-now-true>`, or
  `--amend <NNNN>`. The series is judged as a whole, so an edit in the tree
  that is not in a patch stops the next build as "partly patched".
- Determinism is the product. The guest must not read host time, host
  randomness or anything else that differs between runs; machine time is
  bought with the app's own instructions. A savestate must round-trip. The
  gate checks it; a change that breaks it is a bug.
- The native reference and the sandbox must be the same machine. Both use the
  core's own maths (`mathlib.rs`) and the same pinned Mesa; `build-guest.sh`
  fails if musl's maths reaches the core.
- A stack the guest makes for itself must be mapped with `MAP_STACK` (patch
  0009), or the package dies on Windows while every Linux leg passes.
- Run the gate before committing. A new leg needs a negative control: break
  the thing it checks, see the leg fail, and say so in the commit message.
  Read Chimera's `docs/gates.md` before writing a leg.
- Never commit an `.ipa`, an Apple library or any other game file. Test
  content is built from source; the user's apps stay in `tests/roms-local/`.
- Never add network access.
- CI runs the shell scripts directly. Keep them executable (git mode 100755).
- Documentation prose is plain ASCII.
- Commit messages follow the log: `type: a sentence saying what is now true`,
  for example `fix: every coroutine runs on a declared stack, so Windows can
  deliver a fault (chimera#177)`. Types in use: `feat`, `fix`, `build`; a
  scope is optional (`fix(ci): ...`). The body says what changed, why, how it
  was proven and the gate's tally.
- Do not edit `.github/workflows` unless the task is the workflow.
- Problems with this core are reported in Chimera's issue tracker, not here.

## Where to read more

- `docs/BUILDING.md` - requirements, every script's options, the gate's legs,
  troubleshooting.
- `docs/PLAN.md` - the decisions and the reasoning behind them.
- In the Chimera checkout: `README.md` ("Building"),
  `docs/porting-a-core.md`, `docs/core-manager.md`, `docs/gates.md`.
