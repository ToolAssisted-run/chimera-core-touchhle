#!/bin/bash
# Builds core.wbx: the touchHLE core for the miniBox sandbox.
#
#  1. cargo (nightly, -Z build-std) builds waterbox/guest for the guest target
#     (guest/waterbox-guest.json: musl, large code model, no red zone, no
#     native TLS). touchHLE's C and C++ pieces - dynarmic, OpenAL Soft, the
#     image decoders - are built by its own build scripts, which are handed
#     the guest compilers here through the cc and cmake crates' environment.
#  2. The staticlib is linked with the guest Mesa (setup-mesa.sh), the C++
#     runtime of miniBox's C++ guest kit and the syscall shims into core.wbx,
#     which miniBox's check-wbx.sh then inspects.
#
# Usage: ./build-guest.sh -m <miniBox dir> [-j N]
#   (MINIBOX_DIR works too.) Needs: rustup nightly with rust-src; a miniBox
#   checkout built with the C++ guest kit (build/meson-cpp); build/mesa from
#   ./setup-mesa.sh -m <miniBox dir>.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
minibox="${MINIBOX_DIR:-}"
jobs="$(nproc)"
while getopts "m:j:" opt; do
	case "$opt" in
		m) minibox="$OPTARG" ;;
		j) jobs="$OPTARG" ;;
		*) exit 2 ;;
	esac
done
[ -n "$minibox" ] || { echo "build-guest.sh: say where miniBox is (-m <dir> or MINIBOX_DIR)" >&2; exit 1; }
minibox="$(cd "$minibox" && pwd)"
mbc="$minibox/build/meson-cpp"
sr="$mbc/guest-sysroot"
[ -f "$sr/lib/libstdc++.a" ] || { echo "miniBox C++ guest kit missing at $sr" >&2; exit 1; }
gccver="$(basename "$(ls -d "$sr"/include/c++/* | head -1)")"
mesa="$root/build/mesa/build-guest2"
[ -d "$mesa" ] || { echo "no guest Mesa at $mesa - run ./setup-mesa.sh -m $minibox" >&2; exit 1; }

sh "$here/apply-patches.sh"

# The guest compilers, for every build script touchHLE has. The flags are the
# guest kit's (as in guest-toolchain.cmake); the cc crate's own defaults (it
# would add -fPIC) are switched off. thread_local is defined away: the guest
# has no thread pointer, and the machine is one thread.
wb="-specs=$sr/lib/musl-gcc.specs -mcmodel=large -mstack-protector-guard=global -fno-stack-protector -fno-pic -fno-pie -fcf-protection=none -O2"
notls="-Dthread_local= -D_Thread_local= -D__thread="
export CRATE_CC_NO_DEFAULTS=1
export CC_waterbox_guest=gcc CXX_waterbox_guest=g++ AR_waterbox_guest=ar
export CFLAGS_waterbox_guest="$wb $notls"
export CXXFLAGS_waterbox_guest="$wb $notls -nostdinc++ -I$sr/include/c++/$gccver -I$sr/include/c++/$gccver/x86_64-linux-musl"
export CMAKE_TOOLCHAIN_FILE_waterbox_guest="$here/guest-toolchain.cmake"
export CHIMERA_GUEST_NOTLS="$notls"
export MINIBOX_DIR="$minibox"
export BOOST_INCLUDEDIR="$root/extern/ext-boost"
export CMAKE_BUILD_PARALLEL_LEVEL="$jobs"
export CARGO_TARGET_DIR="$root/build/guest"

( cd "$here/guest" && cargo +nightly build --release -j "$jobs" )
lib="$root/build/guest/waterbox-guest/release/libtouchhle_guest.a"

out="$root/build/wbx"
mkdir -p "$out"
g++ -c -specs "$sr/lib/musl-gcc.specs" -mcmodel=large -fno-pic -fno-pie -fno-stack-protector \
	-fcf-protection=none -O2 -nostdinc++ -I"$sr/include/c++/$gccver" \
	-I"$sr/include/c++/$gccver/x86_64-linux-musl" -o "$out/guest-syscalls.o" "$here/guest-syscalls.cpp"
gcc -c -specs "$sr/lib/musl-gcc.specs" -mcmodel=large -fno-pic -fno-pie -fno-stack-protector \
	-O2 -o "$out/libgcc-builtins.o" "$here/guest/libgcc-builtins.c"

# Mesa: the archives refer to each other both ways, so they are linked in a
# group; the osmesa target's own object comes separately because the shared
# library it belongs to cannot be linked for a guest. Mesa declares some
# pthread entry points WEAK, which statically linked resolve to zero, so each
# is forced into the link.
mesa_target="$(find "$mesa/src/gallium/targets/osmesa" -name 'target.c.o' | head -1)"
mesa_archives="$(find "$mesa" -name '*.a' | sort | tr '\n' ' ')"

exports=""
for e in Init GetLoadError SetButton SetAxis FrameAdvance IsRunning GetFrameCount \
	GetMachineTimeNs GetExecutedTicks GetVideoBgra GetVideoWidth GetVideoHeight GetVsyncNumerator \
	GetVsyncDenominator GetAudio GetAudioSampleCount GetSaveDataFileCount \
	GetSaveDataFileName GetSaveDataFileSize GetSaveDataFileBuffer GetMemoryDomainCount \
	GetMemoryDomainName GetMemoryDomainPtr GetMemoryDomainSize GetMemoryDomainWritable; do
	exports="$exports -Wl,-u,$e"
done

g++ -specs "$sr/lib/musl-gcc.specs" -mcmodel=large -fno-pic -fno-pie -fno-stack-protector \
	-static -no-pie -Wl,--eh-frame-hdr,-O2,--no-relax,-z,stack-size=8388608 \
	-T "$minibox/source/guest/linkscript.T" $exports \
	-Wl,-u,pthread_mutexattr_init -Wl,-u,pthread_mutexattr_settype \
	-Wl,-u,pthread_mutexattr_destroy -Wl,-u,pthread_once -Wl,-u,pthread_key_create \
	-Wl,-u,pthread_cond_wait -Wl,-u,pthread_cond_broadcast \
	-o "$out/core.wbx" \
	"$mbc/source/guest/cxxglue.c.o" "$mbc/source/guest/emulibc.c.o" \
	"$out/guest-syscalls.o" "$out/libgcc-builtins.o" \
	"$lib" "$mesa_target" -Wl,--start-group $mesa_archives -Wl,--end-group \
	-L"$sr/lib" -lstdc++ -lgcc -lgcc_eh -lc -lm
sh "$minibox/source/guest/check-wbx.sh" "$out/core.wbx"
echo "built: $out/core.wbx"
