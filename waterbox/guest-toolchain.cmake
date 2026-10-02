# The miniBox waterbox guest as a CMake toolchain: musl + static libstdc++,
# large code model, no PIC, no host libraries. Mirrors the flag set proven by
# the PPSSPP/PCSX2 guest builds.
set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR x86_64)

# Where the guest sysroot is: MINIBOX_SYSROOT names it outright, MINIBOX_DIR
# (what chimera's bundle and build-package.sh hand every core) implies it, and
# only a bare developer checkout falls back to $HOME/chimera. The release
# runner's checkout is not under $HOME, and the first Dolphin bundle failed
# in CMake's compiler test looking for a specs file there.
if(DEFINED ENV{MINIBOX_SYSROOT})
  set(SR "$ENV{MINIBOX_SYSROOT}")
elseif(DEFINED ENV{MINIBOX_DIR})
  set(SR "$ENV{MINIBOX_DIR}/build/meson-cpp/guest-sysroot")
else()
  set(SR "$ENV{HOME}/chimera/extern/chimera-common-minibox/build/meson-cpp/guest-sysroot")
endif()
if(NOT EXISTS "${SR}/lib/musl-gcc.specs")
  message(FATAL_ERROR "miniBox guest sysroot not found at ${SR}: build it (meson setup <miniBox>/build/meson-cpp <miniBox> -Dguest_cpp=true && ninja -C <miniBox>/build/meson-cpp) or set MINIBOX_DIR")
endif()

execute_process(COMMAND gcc -dumpfullversion OUTPUT_VARIABLE GCCVER OUTPUT_STRIP_TRAILING_WHITESPACE)

set(WB "-specs=${SR}/lib/musl-gcc.specs -fvisibility=hidden -mcmodel=large -mstack-protector-guard=global -fno-stack-protector -fno-pic -fno-pie -fcf-protection=none -DCHIMERA_NO_TLS -DSTBI_NO_THREAD_LOCALS -DZ_TLS=")
set(CMAKE_C_FLAGS_INIT "${WB}")
set(CMAKE_CXX_FLAGS_INIT "${WB} -nostdinc++ -I${SR}/include/c++/${GCCVER} -I${SR}/include/c++/${GCCVER}/x86_64-linux-musl")
set(CMAKE_EXE_LINKER_FLAGS_INIT "-static -nostdlib++")

# feature probes must not try to run guest binaries, and full-exe links need
# the wbx entry glue - compile-and-archive is the honest probe
set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)

set(CMAKE_FIND_ROOT_PATH "${SR}")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# Header-only Boost (extern/ext-boost) lives outside the sysroot, where the
# root-path rule above would never look: name it outright.
if(DEFINED ENV{BOOST_INCLUDEDIR})
  set(Boost_INCLUDE_DIR "$ENV{BOOST_INCLUDEDIR}" CACHE PATH "header-only Boost for dynarmic" FORCE)
endif()
