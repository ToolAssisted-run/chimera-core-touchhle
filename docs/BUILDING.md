# Building the touchHLE core

This repository builds touchHLE (iPhone OS 2.x to 4.0 apps) as a sandboxed
guest for [Chimera](https://github.com/ToolAssisted-run/chimera) and packs it
into one file, `touchhle.chimeraCore`. The steps below are the ones
`.github/workflows/chimera.yml` runs on a fresh clone on a public runner; where
the workflow uses a GitHub Action, the manual equivalent is given. Cores are
built on Linux. The same package file works on Linux and on Windows, because
the guest inside it is run by Chimera's sandbox (miniBox) on either.

Placeholders used below:

- `<core>` - the checkout of this repository.
- `<chimera>` - a checkout of https://github.com/ToolAssisted-run/chimera.
- `<minibox>` - `<chimera>/extern/chimera-common-minibox`, a git submodule of
  Chimera: the sandbox host and the guest toolchain.

Commands are run from `<core>` unless a `cd` says otherwise.

## Requirements

**Operating system.** Linux, x86-64. CI runs on GitHub's `ubuntu-latest`.

**apt packages.** The workflow's one job installs:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends meson ninja-build build-essential cmake pkg-config python3 bison flex python3-mako python3-packaging clang ffmpeg mono-complete libgl1-mesa-dev libx11-dev libxext-dev libasound2-dev
```

What the less obvious ones are for:

- `clang` - compiles the gate's test apps for the iPhone
  (`waterbox/build-testapp.py`, `--target=arm-apple-ios`).
- `ffmpeg` - reads the screenshots the gate's engine leg compares.
- `mono-complete` and .NET - build Chimera, for `chimera-run` and the contract
  tests.
- `bison flex python3-mako python3-packaging` - the Mesa builds
  (`waterbox/setup-mesa.sh` uses the system meson only when
  `python3 -c "import mako, packaging"` works).

**Rust.** Through rustup. The workflow runs:

```sh
rustup toolchain install nightly --profile minimal -c rust-src
rustup default nightly
```

- Channel: `nightly`, and it is the default toolchain. The guest needs
  `-Z build-std`, and the native reference is built with the same toolchain.
  The workflow pins no version: it is the nightly rustup resolves on the day.
  There is no `rust-toolchain` file.
- Profile and components: `--profile minimal` plus the `rust-src` component,
  which `-Z build-std` recompiles `std` from.
- Targets: none is added with rustup. The guest target is a custom target
  specification in this repository, `waterbox/guest/waterbox-guest.json`.
- Crate versions are pinned by the committed `Cargo.lock` files
  (`waterbox/guest/Cargo.lock`, `waterbox/run-native/Cargo.lock`).
- rustup itself is not installed by the workflow: the runner image has it.

**.NET SDK 8.0.** The workflow uses `actions/setup-dotnet@v4` with
`dotnet-version: '8.0'`. Chimera's README gives the manual install:

```sh
curl -sSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0
```

**What the scripts fetch or build themselves.**

- `waterbox/setup-mesa.sh` downloads Mesa 24.0.9
  (`https://archive.mesa3d.org/mesa-24.0.9.tar.xz`) with `curl` into
  `build/deps/`, checks its SHA256 and builds it in `build/mesa/`: once for
  the guest and, with `-n`, once for the host. `MESA_TARBALL=<path>` uses a
  tarball already on the machine.
- `waterbox/run-gate.sh` downloads touchHLE's open-source iPhone OS 3.0 SDK
  (`common-3.0.sdk-linux-x86_64.tar.gz`, release v0.3.7 of
  `touchHLE/common-3.0-sdk`) into `build/deps/`, checks its SHA256 and unpacks
  it. The test apps are linked against it.
- cargo downloads the crates named in the lock files.
- The gate's test apps are built from source: touchHLE's own TestApp from the
  submodule, and SoundTest, SaveTest and ClockTest from `waterbox/tests/apps/`.

## Get the sources

This repository, with its submodules (`actions/checkout@v6` with
`submodules: recursive`):

```sh
git clone https://github.com/ToolAssisted-run/chimera-core-touchhle <core>
cd <core>
git submodule update --init --recursive
```

Recursive is required. `extern/touchHLE` is upstream touchHLE, pinned by
commit, and it has submodules of its own (dynarmic, OpenAL Soft, stb).
`extern/ext-boost` carries the Boost headers dynarmic needs.

Chimera (`actions/checkout@v6` of `ToolAssisted-run/chimera` at `main`, with
`submodules: recursive`):

```sh
git clone https://github.com/ToolAssisted-run/chimera <chimera>
cd <chimera>
git submodule update --init --recursive
```

Where the scripts look for these:

- `waterbox/build-package.sh` takes the Chimera checkout from `-r`. Without
  it, it tries `<core>/../chimera`, then `$HOME/chimera`. It takes miniBox from
  `-m` or `MINIBOX_DIR`, else `<chimera>/extern/chimera-common-minibox`.
- `waterbox/build-guest.sh` and `waterbox/run-gate.sh` have no default for
  miniBox: they stop unless `-m <dir>` or `MINIBOX_DIR` is given. So does
  `waterbox/setup-mesa.sh` for the guest flavour.
- `waterbox/run-gate.sh` takes the Chimera checkout only from `-r`. Without it
  the engine leg is skipped.

## Build miniBox

CI builds Chimera first, because the gate's engine leg and the contract tests
need it:

```sh
cd <chimera>
meson setup build/meson-linux --prefix "$PWD/build" --libdir dll
meson compile -C build/meson-linux
meson install -C build/meson-linux
dotnet build source/gui/Chimera.sln -c Release /nodeReuse:false -p:UseSharedCompilation=false
```

Then miniBox, the sandbox host and the guest toolchain:

```sh
mb=<minibox>
[ -f "$mb/build/meson-linux/build.ninja" ] || meson setup "$mb/build/meson-linux" "$mb"
meson compile -C "$mb/build/meson-linux"
[ -f "$mb/build/meson-cpp/build.ninja" ] || meson setup "$mb/build/meson-cpp" "$mb" -Dguest_cpp=true
meson compile -C "$mb/build/meson-cpp"
```

- `build/meson-linux` holds the sandbox host
  (`source/host/libminiboxhost.so`), which `run-wbx` links.
- `build/meson-cpp` holds the C++ guest kit: `guest-sysroot/` with libstdc++,
  and the guest glue objects the core is linked with.

## Build the core

`waterbox/build-package.sh` runs every step in this section that the package
needs. They are listed so each can be run and understood alone.

### 1. The guest Mesa

```sh
bash waterbox/setup-mesa.sh -m <minibox>
```

Mesa's softpipe driver behind OSMesa, without LLVM, cross-built for the guest.
touchHLE draws an app's OpenGL ES 1.1 through it, inside the core.

- Options: `-n` builds the host flavour instead (no miniBox needed),
  `-m <miniBox dir>`, `-j N`.
- Environment: `MESA_TARBALL=<path>`; `MESA_BUILD_CONFIGURE_ONLY=1` stops after
  configuring; `CHIMERA_DEPS_DIR` moves the download cache; `MESA_BUILD_VENV`
  names the Python venv used when the system meson lacks Mako.
- Output: `build/mesa/build-guest2/` for the guest, `build/mesa/build-native/`
  (with `libOSMesa.so`) for the host. A second run prints
  `mesa: already built` and exits.
- For the guest, Mesa's own last step, linking a shared `libOSMesa`, fails.
  That is expected: the script tolerates it and then checks the archives and
  the osmesa `target.c.o` exist.

CI caches `build/mesa` with `actions/cache@v4`, keyed on the script and the
miniBox commit.

### 2. The patches

Upstream is not modified in git. The changes this core needs are the numbered
files in `patches/`, applied to the working tree of `extern/touchHLE` by
`waterbox/apply-patches.sh`. `build-guest.sh` runs it first. After a build
`git status` shows `extern/touchHLE` as modified. That is the applied series,
and it is normal.

What the script does, in order:

1. It checks that `extern/touchHLE` is a checked-out repository of its own.
   If not, it prints the `git submodule update` command and exits 1.
2. With no `patches/*.patch` at all it prints `no patches to apply` and exits.
3. It lists every file the series touches and copies those files, as the
   submodule's HEAD has them, into a scratch directory.
4. It applies every patch, in order, to the scratch copy with `git apply`. If
   one does not apply it stops with `the series does not apply to the
   submodule's HEAD at <patch>`: the submodule was moved without rebasing the
   patches. The real tree is not touched.
5. If the working tree is pristine (every touched file equals HEAD), it applies
   each patch to the tree and prints `applied: <patch>`.
6. Otherwise the tree must be exactly what the whole series leaves behind. If
   it is, the script prints `already applied: all N patches` and exits 0.
7. If it is neither, it names each file that differs, says
   `extern/touchHLE is partly patched`, and exits 1. It applies nothing.

To start again from the submodule's HEAD (this discards edits made in the
tree):

```sh
git -C extern/touchHLE reset --hard && git -C extern/touchHLE clean -fd && waterbox/apply-patches.sh
```

`TOUCHHLE_TREE=<dir>` points the script at another checkout, for testing the
script itself.

Patches are written with `waterbox/make-patch.py`, never with a bare
`git diff`. Its baseline is HEAD plus the existing patches, so a file two
patches touch does not end up in both. Edit the files under
`extern/touchHLE`, do not commit there, then:

```sh
python3 waterbox/make-patch.py --check
python3 waterbox/make-patch.py <NNNN-chimera-what-is-now-true>
python3 waterbox/make-patch.py --amend <NNNN>
```

- `--check` lists the files a new patch would hold and writes nothing.
- A name writes `patches/<name>.patch`: whatever the tree changes beyond what
  the series already leaves behind.
- `--amend <NNNN>` rewrites patch NNNN from the tree: its own files, against
  HEAD plus the patches before it. A file that a later patch also touches
  keeps its hunks as they are and cannot be amended this way.

### 3. The guest core

```sh
bash waterbox/build-guest.sh -m <minibox>
```

Options: `-m <miniBox dir>` (or `MINIBOX_DIR`), `-j N`. It needs
`build/mesa/build-guest2` and stops without it.

What it does:

1. Applies the patches.
2. Exports the guest compilers for the build scripts inside touchHLE's
   dependency tree (the `cc` and `cmake` crates read `CC_waterbox_guest`,
   `CFLAGS_waterbox_guest`, `CMAKE_TOOLCHAIN_FILE_waterbox_guest` and so on).
   dynarmic, OpenAL Soft and the image decoders are built that way.
   `waterbox/guest-toolchain.cmake` is the CMake toolchain file, and
   `BOOST_INCLUDEDIR` is set to `extern/ext-boost`.
3. Runs `cargo +nightly build --release -j N` from `waterbox/guest`, with
   `CARGO_TARGET_DIR=build/guest`. The directory's `.cargo/config.toml` sets
   `build-std = ["std", "panic_abort"]`, `target = "waterbox-guest.json"` and
   the rustflags
   `-C code-model=large -C relocation-model=static -C target-feature=+crt-static`.
   `waterbox-guest.json` is the stock `x86_64-unknown-linux-musl` target with
   `has-thread-local` off and `disable-redzone` on. The result is
   `build/guest/waterbox-guest/release/libtouchhle_guest.a`.
4. Links it with the guest Mesa, miniBox's C++ runtime and the syscall shims
   into `build/wbx/core.wbx`, and runs miniBox's `check-wbx.sh` on it.
5. Fails if one of musl's maths helpers reached the core. The maths must be
   the core's own (`waterbox/guest/src/mathlib.rs`) in both builds.

### 4. The native reference

There is no separate script. `waterbox/run-gate.sh` builds it, with:

```sh
bash waterbox/setup-mesa.sh -n
(cd waterbox/run-native && CARGO_TARGET_DIR=<core>/build/run-native cargo build --release)
```

- Output: `build/run-native/release/run-native`.
- It uses the default toolchain, which CI sets to nightly.
- What it is for: it is the core's own crate (`waterbox/guest`) built for the
  host and driven the way the engine drives `core.wbx`, with no sandbox. Every
  gate leg compares it with the sandboxed core. It is also where debugging is
  done.
- It links the host build of the same pinned Mesa, never the host's own
  OpenGL: an app can query its OpenGL, so the OpenGL is part of the machine.
  Its `build.rs` stops with `no host Mesa at ... - run waterbox/setup-mesa.sh
  -n first` when that build is missing. `MESA_NATIVE_DIR` names another one.
- It needs the patches applied: the `chimera` feature it builds touchHLE with
  comes from them. `run-gate.sh` applies them before it builds the
  reference; building the reference by hand on a fresh clone, run
  `waterbox/apply-patches.sh` first.

## Build the package

```sh
./waterbox/build-package.sh -m <minibox> -r <chimera>
```

Options: `-m <miniBox dir>` (or `MINIBOX_DIR`) and `-r <chimera root>`. There
is no `-o`: the output location follows `-r`.

What it does:

1. Runs `setup-mesa.sh -m` and `build-guest.sh -m`: the guest Mesa and the
   core. It does not build the native reference.
2. Stages a copy of `build/wbx/core.wbx` in `build/package-staging/` and
   strips its debug sections (`strip --strip-debug`). The symbol table stays.
   `build/wbx/core.wbx` keeps everything, for a debugger.
3. Adds `waterbox.config`, `default_keybinds.json`, `file_slots.json` and the
   licence texts that miniBox's `package-licenses.py` gathers from
   `waterbox/package-licenses.json`.
4. Stamps the version into the staged `waterbox.config` and writes
   `build.json`, which records what built the package.
5. Zips the staging directory deterministically, twice, and fails if the two
   SHA1s differ. It prints `package sha1 <hash>`.
6. Writes `<chimera>/build/Cores/touchhle.chimeraCore`, removes any
   `<chimera>/build/CoreCache/touchhle-*` directory, and prints
   `packaged -> <path>`.

**The version stamp.** A package's version is the commit it was built from. CI
sets `CORE_VERSION` to the commit (`${{ github.sha }}`) for this step. Without
`CORE_VERSION` the script stamps `<commit>+local`, where `<commit>` is the
12-character short hash, and `<commit>-dirty+local` when `git diff --quiet
HEAD` reports changes. The applied patches count as a change to
`extern/touchHLE`, so a hand build normally carries `-dirty`. The commit's
date, in UTC, is stamped beside it as `versionDate`. Hand-built packages are
for testing: Chimera's publishing script refuses a version that carries
`+local` or `-dirty`.

**Releases.** On every green push to `main` the workflow's `publish` job
replaces the rolling `dev` release. The scheduled run (cron `0 4 * * *`)
publishes a dated `nightly-YYYY-MM-DD` release, only when `main` moved since
the last one. Nothing is published from a pull request.

## Install it into Chimera

Chimera ships no cores and downloads nothing: it has no network code. A user
downloads a core's `.chimeraCore` package from the core repository's Releases
page, or builds it, and puts it in Chimera's `Cores` folder.

- In a Chimera source checkout the cores folder is `<chimera>/build/Cores/`.
  `waterbox/build-package.sh -r <chimera>` writes the package straight there,
  so there is nothing more to do.
- In a release bundle it is the `Cores` folder beside `Chimera.exe`, or another
  folder chosen in File > Core Manager > Change folder... Copy
  `touchhle.chimeraCore` into it.
- File > Core Manager lists what is in that folder. Refresh List rescans it.

Published packages are at
https://github.com/ToolAssisted-run/chimera-core-touchhle/releases.

## Run the gates

### The gate

```sh
./waterbox/run-gate.sh -m <minibox> -r <chimera>
```

Options:

- `-m <miniBox dir>` (or `MINIBOX_DIR`) - required.
- `-r <chimera root>` - a Chimera checkout with
  `build/meson-linux/chimera-run` and the package in `build/Cores`, for the
  engine leg.
- `-n` - do not rebuild; use what `build/` holds.
- `-j N`.

Without `-n` it first builds what the package build did not: the host Mesa,
the guest Mesa, the native reference, the core, `build/wbx/run-wbx`, the SDK
and the test apps (`build/testapp/*.ipa`). The build logs and every leg's
output are in `build/gate/`. Where `systemd-run --user --scope` works, each
sandboxed run is held under an 8G memory cap.

Every leg compares two runs that must agree, or checks a thing that must hold:

- TestApp - native and sandbox give the same digests (ticks, time, picture,
  sound) over 300 frames, and the screen moves.
- tap - a finger on the screen: native equals sandbox; a savestate saved and
  loaded before every frame changes nothing (`--rerecord`); a state moved to a
  fresh host at frame 120 lands the same (`--session`); and the tap is seen.
- CLI tests - TestApp's 105 command-line tests pass natively and in the
  sandbox, with identical output, and the app's exit stops the machine.
- clock (ClockTest) - native equals sandbox; a stalled host changes nothing;
  the `cpu_mhz` setting is what buys time; the date starts at `rtc_start`.
- sound (SoundTest) - silence, then one tone, then two; native equals sandbox;
  the same through a savestate every frame and through a fresh host.
- save data (SaveTest) - the export is the same in both flavours, and an app
  started from its export carries on.
- the engine - `chimera-run` plays a tap movie on the package and its picture
  is `run-wbx`'s byte for byte. **SKIP** without `-r` naming a checkout that
  has `chimera-run` and the package.
- games - one group of legs for every decrypted `.ipa` in
  `tests/roms-local/`: native equals sandbox, a state moved to a fresh host
  lands the same, and the run goes where it should.
  `waterbox/tests/game-list.txt` gives each app its frames and input; an
  `.ipa` it does not name runs untouched for 900 frames. **SKIP** when
  `tests/roms-local/` holds no `.ipa`, which is the case in CI. A line whose
  frames field is `SKIP` is reported as SKIP too.

The last line is `gate: N passed, N failed, N skipped`, and the script exits
non-zero if anything failed. Every leg except the games runs in CI.

### Chimera's contract tests

Run against the package just built: it must be readable, built for an ABI this
frontend runs, become a working factory, bind only buttons its controller
declares, and stamp a version.

```sh
cd <chimera>
CHIMERA_CORES_DIR=<chimera>/build/Cores dotnet test source/gui/Chimera.Tests.Client.Common/Chimera.Tests.Client.Common.csproj -c Release --nologo --filter "FullyQualifiedName~InstalledCorePackagesTests|FullyQualifiedName~MnemonicUniquenessTests"
```

## Files the core needs at run time

Game files are never in this repository or in the package. The user provides
them. This core needs no BIOS and no firmware: touchHLE reimplements iPhone OS,
and the core carries the free libraries and fonts apps load. A project's
files, from `waterbox/file_slots.json`:

- **App** - one `.ipa`, required. It must be a DECRYPTED `.ipa` for iPhone OS
  2.x to iOS 4.0. An App Store download is encrypted and will not run.
  touchHLE's compatibility database (https://appdb.touchhle.org/) says which
  apps and versions work.
- **Save data** - one `.zip`, optional: what the app already saved, as
  Emulator > Export Save Data... wrote it. It goes back into the app's home
  before the app starts.

## Troubleshooting

- `build-guest.sh: say where miniBox is (-m <dir> or MINIBOX_DIR)`, and the
  same from `run-gate.sh` - these two scripts have no default. Pass `-m`.
- `miniBox C++ guest kit missing at ...` or `guest sysroot not built at ...
  (build miniBox's meson-cpp first)` - miniBox's `build/meson-cpp` is not
  built.
- `no guest Mesa at ... - run ./setup-mesa.sh -m ...` - build the guest Mesa.
- `no host Mesa at ... - run waterbox/setup-mesa.sh -n first` - the native
  reference wants the host flavour.
- `extern/touchHLE is not checked out` - run
  `git submodule update --init --recursive` in `<core>`.
- `extern/touchHLE is partly patched` - see "The patches" above.
- `the series does not apply to the submodule's HEAD at <patch>` - the
  submodule pin moved and the patches were not rebased.
- The native reference fails to build by hand on a fresh clone - the patches
  are not applied yet. See "The native reference" above.
- `build-guest.sh: musl's maths reached the core (...)` - a maths function
  came from musl. Add it to `waterbox/guest/src/mathlib.rs`.
- `the build failed; see build/gate/build-*.log` - `run-gate.sh` sends each
  build step's output to a log there.
- `the SDK is not the release this gate pins` - the tarball in `build/deps/`
  does not have the pinned SHA256.
- `missing <file> (run without -n)` - `-n` was given before anything was
  built.
- `mesa: no usable meson - install python3-venv, or meson plus python3-mako` -
  install the apt packages above.
- `mesa: the tarball is not the release this core is pinned to` - the file in
  `build/deps/` (or `MESA_TARBALL`) is not Mesa 24.0.9 with the pinned SHA256.
- A package that passes every Linux leg can still die on Windows in its first
  frame if guest code runs on a stack that was not mapped with `MAP_STACK`:
  Windows cannot deliver a fault there. Patch 0009 puts every coroutine on a
  declared stack; `docs/PLAN.md` has the account.
