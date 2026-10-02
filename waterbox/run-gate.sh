#!/bin/bash
# The touchHLE core's gate: every leg compares two runs that must agree, or
# checks a thing that must hold, and says what it compared.
#
# Content: touchHLE's own TestApp (its UIKit screen and its 105 CLI tests)
# and this repository's apps (waterbox/tests/apps: SoundTest, SaveTest,
# ClockTest), all
# built from source by build-testapp.py - nothing that cannot be distributed.
# Games, when there are any, are the decrypted .ipa files in tests/roms-local/
# (SKIP without them).
#
# Usage: ./run-gate.sh -m <miniBox dir> [-r <chimera root>] [-n] [-j N]
#   -n   do not rebuild (use what build/ holds)
#   -r   a chimera checkout with build/meson-linux/chimera-run, for the engine leg
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
minibox="${MINIBOX_DIR:-}"
chimera_root=""
jobs="$(nproc)"
rebuild=1
while getopts "m:r:nj:" opt; do
	case "$opt" in
		m) minibox="$OPTARG" ;;
		r) chimera_root="$OPTARG" ;;
		n) rebuild=0 ;;
		j) jobs="$OPTARG" ;;
		*) exit 2 ;;
	esac
done
[ -n "$minibox" ] || { echo "run-gate.sh: say where miniBox is (-m <dir> or MINIBOX_DIR)" >&2; exit 2; }
minibox="$(cd "$minibox" && pwd)"

build="$root/build"
work="$build/gate"
mkdir -p "$work"
native="$build/run-native/release/run-native"
wbx="$build/wbx/run-wbx"
core="$build/wbx/core.wbx"
apps="$build/testapp"
sdk="$build/deps/sdk/common-3.0.sdk"

pass=0
fail=0
skip=0
report() {
	local verdict="$1" what="$2" detail="${3:-}"
	case "$verdict" in
		PASS) pass=$((pass + 1)) ;;
		FAIL) fail=$((fail + 1)) ;;
		SKIP) skip=$((skip + 1)) ;;
	esac
	printf '%-4s %s%s\n' "$verdict" "$what" "${detail:+ - $detail}"
}

# ------------------------------------------------------------------ building
if [ "$rebuild" = 1 ]; then
	echo "building (the logs are in $work/)"
	export BOOST_INCLUDEDIR="$root/extern/ext-boost"
	bash "$here/setup-mesa.sh" -n -j "$jobs" > "$work/build-mesa-native.log" 2>&1 &&
	bash "$here/setup-mesa.sh" -m "$minibox" -j "$jobs" > "$work/build-mesa-guest.log" 2>&1 &&
	(cd "$here/run-native" && CARGO_TARGET_DIR="$build/run-native" cargo build --release -j "$jobs") \
		> "$work/build-native.log" 2>&1 &&
	bash "$here/build-guest.sh" -m "$minibox" -j "$jobs" > "$work/build-guest.log" 2>&1 &&
	cc -O2 -Wall -I"$minibox/source/host" -o "$wbx" "$here/run-wbx.c" \
		"$minibox/build/meson-linux/source/host/libminiboxhost.so" \
		-Wl,-rpath,"$minibox/build/meson-linux/source/host" > "$work/build-run-wbx.log" 2>&1 || {
		echo "the build failed; see $work/build-*.log" >&2
		exit 1
	}
	if [ ! -d "$sdk" ]; then
		mkdir -p "$build/deps/sdk"
		tgz="$build/deps/common-3.0.sdk-linux-x86_64-v0.3.7.tar.gz"
		[ -f "$tgz" ] || curl -fsSL -o "$tgz" \
			https://github.com/touchHLE/common-3.0-sdk/releases/download/v0.3.7/common-3.0.sdk-linux-x86_64.tar.gz
		echo "b72fdbe2f9104ee4886fb76a977fd617cb5d5b168b4225e1b4fd582d99c65e9b  $tgz" | sha256sum -c - >/dev/null ||
			{ echo "the SDK is not the release this gate pins" >&2; exit 1; }
		tar -xzf "$tgz" -C "$build/deps/sdk"
	fi
	python3 "$here/build-testapp.py" --touchhle "$native" --sdk "$sdk" \
		> "$work/build-testapp.log" 2>&1 || { echo "building the test apps failed" >&2; exit 1; }
fi
for f in "$native" "$wbx" "$core" "$apps/TestApp.ipa" "$apps/SoundTest.ipa" "$apps/SaveTest.ipa" "$apps/ClockTest.ipa"; do
	[ -e "$f" ] || { echo "missing $f (run without -n)" >&2; exit 1; }
done

# a sandboxed run under a memory cap where systemd allows one (not on CI)
if systemd-run --user --scope -q true 2>/dev/null; then
	box() { systemd-run --user --scope -q -p MemoryMax=8G "$@"; }
else
	box() { "$@"; }
fi
# digest lines only: what the app prints on stdout is not the machine's state
digest() { grep -E '^(frame |frames=|save )' "$1"; }

# ------------------------------------------------ 1. TestApp: native == sandbox
"$native" "$apps/TestApp.ipa" --frames 300 --digest-every 30 > "$work/ui.n" 2> "$work/ui.ne"
box "$wbx" "$core" "$apps/TestApp.ipa" --frames 300 --digest-every 30 > "$work/ui.w" 2> "$work/ui.we"
if [ "$(digest "$work/ui.n" | wc -l)" = 11 ] && diff <(digest "$work/ui.n") <(digest "$work/ui.w") > /dev/null; then
	report PASS "TestApp: native == sandbox" "300 frames, ticks/time/picture/sound every 30"
else
	report FAIL "TestApp: native == sandbox" "see $work/ui.n and ui.w"
fi
pictures="$(digest "$work/ui.n" | awk '/^frame /{print $4}' | sort -u | wc -l)"
if [ "$pictures" -ge 5 ]; then
	report PASS "TestApp: the screen moves" "$pictures different pictures in 10 digests (its animation)"
else
	report FAIL "TestApp: the screen moves" "only $pictures different pictures"
fi

# ------------------------------------------- 2. a tap, through every flavor
X=$((160 * 65536 / 320 + 102))
Y=$((260 * 65536 / 480 + 68))
tap=(--frames 240 --touch "120:1:$X:$Y" --release 124:1)
"$native" "$apps/TestApp.ipa" "${tap[@]}" --screenshot 239="$work/tap.tga" > "$work/tap.n" 2>/dev/null
box "$wbx" "$core" "$apps/TestApp.ipa" "${tap[@]}" --screenshot 239="$work/tap-wbx.tga" > "$work/tap.w" 2>/dev/null
box "$wbx" "$core" "$apps/TestApp.ipa" "${tap[@]}" --rerecord > "$work/tap.r" 2>/dev/null
box "$wbx" "$core" "$apps/TestApp.ipa" "${tap[@]}" --session > "$work/tap.s" 2>/dev/null
"$native" "$apps/TestApp.ipa" --frames 240 > "$work/notap.n" 2>/dev/null
ref="$(digest "$work/tap.n")"
if [ -n "$ref" ] && [ "$ref" = "$(digest "$work/tap.w")" ]; then
	report PASS "tap: native == sandbox" "a finger on 'CALayer tests' at 120..123"
else
	report FAIL "tap: native == sandbox"
fi
if [ -n "$ref" ] && [ "$ref" = "$(digest "$work/tap.r")" ]; then
	report PASS "tap: a savestate saved and loaded before every frame changes nothing"
else
	report FAIL "tap: rerecord" "see $work/tap.r"
fi
if [ -n "$ref" ] && [ "$ref" = "$(digest "$work/tap.s")" ]; then
	report PASS "tap: the state moved to a fresh host at frame 120 lands the same"
else
	report FAIL "tap: session" "see $work/tap.s"
fi
if [ -n "$ref" ] && [ "$(awk '{print $4}' <<< "$ref")" != "$(digest "$work/notap.n" | awk '{print $4}')" ]; then
	report PASS "tap: the tap is seen" "the picture after it differs from the untouched run's"
else
	report FAIL "tap: the tap is seen" "the tapped and untouched runs end on the same picture"
fi

# ------------------------------------------- 3. TestApp's CLI tests, both flavors
"$native" "$apps/TestApp.ipa" --frames 3000 --setting appArgs=--cli-tests > "$work/cli.n" 2>/dev/null
box "$wbx" "$core" "$apps/TestApp.ipa" --frames 3000 --setting appArgs=--cli-tests > "$work/cli.w" 2>/dev/null
for flavor in n w; do
	if grep -q "^Passed 105 out of 105 tests" "$work/cli.$flavor"; then
		report PASS "CLI tests ($([ $flavor = n ] && echo native || echo sandbox)): 105/105"
	else
		report FAIL "CLI tests ($([ $flavor = n ] && echo native || echo sandbox))" \
			"$(grep -a "^Passed" "$work/cli.$flavor" || echo 'no result')"
	fi
done
if cmp -s "$work/cli.n" "$work/cli.w" && grep -q "running=0" "$work/cli.n"; then
	report PASS "CLI tests: native == sandbox, and the app's exit stops the machine" "every line"
else
	report FAIL "CLI tests: native == sandbox, and the app's exit stops the machine"
fi

# ------------------------------------------- 4. the clock is the machine's own
# ClockTest reads mach_absolute_time() in a busy loop for a quarter of a
# second and prints time(), once a second: the count follows cpu_mhz, the
# date rtc_start, and neither may follow the host.
clock() { grep -a '^ClockTest:' "$1"; }
"$native" "$apps/ClockTest.ipa" --frames 200 > "$work/clock.n" 2>/dev/null
box "$wbx" "$core" "$apps/ClockTest.ipa" --frames 200 > "$work/clock.w" 2>/dev/null
"$native" "$apps/ClockTest.ipa" --frames 200 --stall 30:300 --stall 100:300 > "$work/clock-stall.n" 2>/dev/null
"$native" "$apps/ClockTest.ipa" --frames 200 --setting cpu_mhz=600 > "$work/clock-600.n" 2>/dev/null
"$native" "$apps/ClockTest.ipa" --frames 200 --setting rtc_start=1000000000 > "$work/clock-date.n" 2>/dev/null
cref="$(clock "$work/clock.n")"
if [ "$(wc -l <<< "$cref")" -ge 3 ] && [ "$cref" = "$(clock "$work/clock.w")" ] && cmp -s <(digest "$work/clock.n") <(digest "$work/clock.w"); then
	report PASS "clock: native == sandbox" "$(sed -n 1p <<< "$cref" | cut -d' ' -f3-)"
else
	report FAIL "clock: native == sandbox" "see $work/clock.n and clock.w"
fi
if [ -n "$cref" ] && [ "$cref" = "$(clock "$work/clock-stall.n")" ] && cmp -s <(digest "$work/clock.n") <(digest "$work/clock-stall.n"); then
	report PASS "clock: the host stalling 300 ms twice changes nothing" "the app's clock readings included"
else
	report FAIL "clock: a host stall changed the machine" "see $work/clock-stall.n"
fi
r412="$(sed -n 1p <<< "$cref" | awk '{print $7}')"
r600="$(clock "$work/clock-600.n" | sed -n 1p | awk '{print $7}')"
if [ -n "$r412" ] && [ -n "$r600" ] && [ "$r600" -gt $((r412 * 14 / 10)) ] && [ "$r600" -lt $((r412 * 15 / 10)) ]; then
	report PASS "clock: the CPU clock is what buys time" "$r412 reads a quarter second at 412 MHz, $r600 at 600 (the stall check can fail)"
else
	report FAIL "clock: cpu_mhz" "$r412 reads at 412 MHz, ${r600:-none} at 600"
fi
if [ "$(clock "$work/clock-date.n" | sed -n 3p | awk '{print $5}')" = 1000000002 ] \
	&& [ "$(sed -n 3p <<< "$cref" | awk '{print $5}')" = 1262304002 ]; then
	report PASS "clock: the date starts at rtc_start and runs with the machine" "time() 2 s in: 1262304002, and 1000000002 from 1000000000"
else
	report FAIL "clock: rtc_start" "$(sed -n 3p <<< "$cref")"
fi

# ------------------------------------------- 5. sound
"$native" "$apps/SoundTest.ipa" --frames 240 --digest-every 30 > "$work/snd.n" 2>/dev/null
box "$wbx" "$core" "$apps/SoundTest.ipa" --frames 240 --digest-every 30 > "$work/snd.w" 2>/dev/null
box "$wbx" "$core" "$apps/SoundTest.ipa" --frames 240 --rerecord > "$work/snd.r" 2>/dev/null
box "$wbx" "$core" "$apps/SoundTest.ipa" --frames 240 --session > "$work/snd.s" 2>/dev/null
peaks="$(digest "$work/snd.n" | awk '/^frame /{sub("peak=","",$7); print $7}' | tr '\n' ' ')"
read -r p30 p60 p90 p120 p150 _ <<< "$peaks"
if [ "${p30:-9}" -le 1 ] && [ "${p60:-9}" -le 1 ] && [ "${p90:-0}" -gt 1000 ] && [ "${p150:-0}" -gt "${p120:-0}" ]; then
	report PASS "sound: silence, then one tone, then two mixed" "peaks $peaks"
else
	report FAIL "sound: the tones are not where SoundTest plays them" "peaks $peaks"
fi
sref="$(digest "$work/snd.n" | tail -1)"
if [ -n "$sref" ] && diff <(digest "$work/snd.n") <(digest "$work/snd.w") > /dev/null; then
	report PASS "sound: native == sandbox" "every frame's samples hashed"
else
	report FAIL "sound: native == sandbox"
fi
if [ -n "$sref" ] && [ "$sref" = "$(digest "$work/snd.r" | tail -1)" ] && [ "$sref" = "$(digest "$work/snd.s" | tail -1)" ]; then
	report PASS "sound: through a savestate every frame, and through a fresh host"
else
	report FAIL "sound: a savestate changed the sound"
fi

# ------------------------------------------- 6. save data in and out
rm -f "$work"/save-*.zip
"$native" "$apps/SaveTest.ipa" --frames 200 --export-save "$work/save-n.zip" > "$work/save.n" 2>/dev/null
box "$wbx" "$core" "$apps/SaveTest.ipa" --frames 200 --export-save "$work/save-w.zip" > "$work/save.w" 2>/dev/null
"$native" "$apps/SaveTest.ipa" --frames 70 --savedata "$work/save-n.zip" > "$work/save2.n" 2>/dev/null
box "$wbx" "$core" "$apps/SaveTest.ipa" --frames 70 --savedata "$work/save-n.zip" --session > "$work/save2.w" 2>/dev/null
"$native" "$apps/SaveTest.ipa" --frames 70 > "$work/save0.n" 2>/dev/null
if grep -q "^save Documents/counter.txt " "$work/save.n" && diff <(digest "$work/save.n") <(digest "$work/save.w") > /dev/null; then
	report PASS "save data: the export is the same in both flavors" "$(grep '^save ' "$work/save.n" | cut -d' ' -f2-)"
else
	report FAIL "save data: the export" "see $work/save.n and save.w"
fi
if grep -q "found 3" "$work/save2.n" && grep -q "found 3" "$work/save2.w" && grep -q "found 0" "$work/save0.n"; then
	report PASS "save data: an app started from its export carries on" "native, and the sandbox through a fresh host; 0 without it"
else
	report FAIL "save data: the import" "$(grep -h found "$work"/save2.n "$work"/save2.w "$work"/save0.n | tr '\n' ' ')"
fi

# ------------------------------------------- 7. the package, through the engine
run="${chimera_root:+$chimera_root/build/meson-linux/chimera-run}"
pkg="${chimera_root:+$chimera_root/build/Cores/touchhle.chimeraCore}"
if [ -n "$run" ] && [ -x "$run" ] && [ -f "$pkg" ]; then
	python3 - "$work/tap.movie" "$X" "$Y" <<'PY'
import sys
path, x, y = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
out = []
for f in range(240):
    down = 120 <= f < 124
    ax = (x if down else 32768, y if down else 32768, 32768, 32768, 0, 0)
    out.append('|' + ''.join('%5d,' % v for v in ax) + ('1' if down else '.') + '.|')
open(path, 'w').write('\n'.join(out) + '\n')
PY
	box "$run" "$pkg" "$apps/TestApp.ipa" "$work/tap.movie" --screenshot 239="$work/tap-engine.tga" > "$work/engine.out" 2>&1
	a="$(ffmpeg -loglevel error -i "$work/tap-engine.tga" -f rawvideo -pix_fmt rgb24 - 2>/dev/null | sha1sum)"
	b="$(ffmpeg -loglevel error -i "$work/tap-wbx.tga" -f rawvideo -pix_fmt rgb24 - 2>/dev/null | sha1sum)"
	if grep -q '^frames=240' "$work/engine.out" && [ "$a" = "$b" ]; then
		report PASS "the engine: the package plays the tap movie" "its picture is run-wbx's, byte for byte"
	else
		report FAIL "the engine: the package plays the tap movie" "see $work/engine.out"
	fi
else
	report SKIP "the engine" "no chimera checkout with chimera-run and the package (-r)"
fi

# ------------------------------------------- 8. games (tests/roms-local/*.ipa)
# What each app does is waterbox/tests/game-list.txt's: its frames and its
# input, the same for every flavor. An .ipa the list does not name runs
# untouched for 900 frames.
shopt -s nullglob
games=("$root"/tests/roms-local/*.ipa)
if [ "${#games[@]}" = 0 ]; then
	report SKIP "games" "no .ipa in tests/roms-local"
fi
for game in "${games[@]}"; do
	name="$(basename "$game" .ipa)"
	line="$(grep -E "^$name *\|" "$here/tests/game-list.txt" | head -1)"
	frames=900
	input=()
	if [ -n "$line" ]; then
		IFS='|' read -r _ frames inputs shows <<< "$line"
		frames="$(echo $frames)"
		read -r -a input <<< "$inputs"
		if [ "$frames" = SKIP ]; then
			report SKIP "$name" "$(echo $shows)"
			continue
		fi
	fi
	half=$((frames / 2))
	t0=$(date +%s)
	"$native" "$game" --frames "$frames" --digest-every 300 "${input[@]}" > "$work/g-$name.n" 2> "$work/g-$name.ne"
	t1=$(date +%s)
	box "$wbx" "$core" "$game" --frames "$frames" --digest-every 300 "${input[@]}" > "$work/g-$name.w" 2> "$work/g-$name.we"
	t2=$(date +%s)
	box "$wbx" "$core" "$game" --frames "$frames" --digest-every 300 "${input[@]}" --session > "$work/g-$name.s" 2>/dev/null
	speed="native $((frames / (t1 - t0 + 1))) fps, sandbox $((frames / (t2 - t1 + 1))) fps"
	if [ "$(digest "$work/g-$name.n" | grep -c '^frame ')" -ge 2 ] && grep -q "^frames=$frames .*running=1" "$work/g-$name.n" \
		&& diff <(digest "$work/g-$name.n") <(digest "$work/g-$name.w") > /dev/null; then
		report PASS "$name: native == sandbox" "$frames frames, a digest every 300; $speed"
	else
		report FAIL "$name: native == sandbox" "first difference: $(diff <(digest "$work/g-$name.n") <(digest "$work/g-$name.w") | sed -n 2p | cut -c1-80)"
	fi
	if [ -s "$work/g-$name.w" ] && diff <(digest "$work/g-$name.w") <(digest "$work/g-$name.s") > /dev/null; then
		report PASS "$name: the state moved to a fresh host at $half lands the same"
	else
		report FAIL "$name: session" "see $work/g-$name.s"
	fi
	pictures="$(digest "$work/g-$name.n" | awk '/^frame /{print $4}' | sort -u | wc -l)"
	if [ "${#input[@]}" = 0 ] || [ "$pictures" -ge 4 ]; then
		report PASS "$name: the run goes where it should" "$pictures different pictures in $(digest "$work/g-$name.n" | grep -c '^frame ') digests: $(echo ${shows:-no input})"
	else
		report FAIL "$name: the run goes where it should" "only $pictures different pictures"
	fi
done

echo
echo "gate: $pass passed, $fail failed, $skip skipped"
[ "$fail" = 0 ]
