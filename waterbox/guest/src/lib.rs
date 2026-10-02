//! The touchHLE core: the exports the Chimera engine calls, over touchHLE's
//! frame-at-a-time machine (extern/touchHLE, feature "chimera").
//!
//! The same code is the native reference: waterbox/run-native links this crate
//! for the host and calls these functions in the order the engine does, so the
//! two flavors differ only in where they run.

#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::CString;
use touchHLE::chimera;

// ---- The controller, in waterbox.config's order ---------------------------

/// Buttons: a finger touching the screen.
pub const BUTTONS: &[&str] = &["Touch 1", "Touch 2"];
/// Axes: where each finger is on the picture (0..65535 across and down, as
/// every absolute position in Chimera is), and how the device is tilted
/// (-32768..32767, what an analog stick does in upstream touchHLE).
pub const AXES: &[&str] = &["Touch 1 X", "Touch 1 Y", "Touch 2 X", "Touch 2 Y", "Tilt X", "Tilt Y"];
const AXIS_TILT_X: usize = 4;
const AXIS_TILT_Y: usize = 5;

/// 44.1 kHz stereo at 60 frames a second.
pub const AUDIO_RATE: u32 = 44100;
pub const AUDIO_PAIRS_PER_FRAME: usize = (AUDIO_RATE / 60) as usize;

struct Core {
    /// Why Init refused, NUL-terminated.
    load_error: Vec<u8>,
    buttons: u64,
    axes: [i32; 6],
    audio: Vec<i16>,
    running: bool,
}
static mut CORE: Core = Core {
    load_error: Vec::new(),
    buttons: 0,
    axes: [32768, 32768, 32768, 32768, 0, 0],
    audio: Vec::new(),
    running: false,
};

#[allow(static_mut_refs)]
fn core() -> &'static mut Core {
    unsafe { &mut CORE }
}

fn fail(why: String) -> i32 {
    eprintln!("touchhle core: {why}");
    core().load_error = CString::new(why.replace('\0', " ")).unwrap().into_bytes_with_nul();
    0
}

// ---- Settings -----------------------------------------------------------------

/// The settings the engine mounted ("settings", a flat JSON object), as
/// touchHLE options and machine parameters.
pub struct Settings {
    /// touchHLE command-line options.
    pub options: Vec<String>,
    /// Seconds since the Unix epoch when the machine starts.
    pub start_date: u64,
    /// The CPU clock, Hz.
    pub cpu_hz: u64,
}

impl Settings {
    pub fn parse(json: &str) -> Result<Settings, String> {
        let mut s = Settings {
            options: Vec::new(),
            start_date: 946_684_800, // 2000-01-01
            cpu_hz: 412_000_000,
        };
        for (key, value) in flat_json(json)? {
            match key.as_str() {
                "deviceFamily" => match value.as_str() {
                    "auto" => {}
                    "iphone" | "ipad" => s.options.push(format!("--device-family={value}")),
                    _ => return Err(format!("deviceFamily: unknown value {value:?}")),
                },
                "orientation" => match value.as_str() {
                    "portrait" => {}
                    "upside-down" | "landscape-left" | "landscape-right" => {
                        s.options.push(format!("--{value}"))
                    }
                    _ => return Err(format!("orientation: unknown value {value:?}")),
                },
                "startDate" => {
                    s.start_date = parse_date(&value).ok_or_else(|| {
                        format!("startDate: {value:?} is not a date (YYYY-MM-DD, 1970 or later)")
                    })?
                }
                "cpuMHz" => {
                    let mhz: u64 = value
                        .parse()
                        .ok()
                        .filter(|&m| (1..=10_000).contains(&m))
                        .ok_or_else(|| format!("cpuMHz: {value:?} is not a clock"))?;
                    s.cpu_hz = mhz * 1_000_000;
                }
                "languages" => {
                    if !value.is_empty() {
                        s.options.push(format!("--preferred-languages={value}"))
                    }
                }
                _ => {} // a setting this build does not know is not an error
            }
        }
        Ok(s)
    }
}

/// A flat JSON object's members, values as text (strings unquoted). The
/// settings channel is never nested.
fn flat_json(json: &str) -> Result<Vec<(String, String)>, String> {
    let bad = || format!("settings are not a flat JSON object: {json:?}");
    let s = json.trim();
    let inner = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')).ok_or_else(bad)?;
    let mut out = Vec::new();
    let mut chars = inner.chars().peekable();
    let string = |chars: &mut std::iter::Peekable<std::str::Chars>| -> Option<String> {
        let mut v = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(v),
                '\\' => match chars.next()? {
                    'n' => v.push('\n'),
                    't' => v.push('\t'),
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| chars.next()).collect();
                        v.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    c => v.push(c),
                },
                c => v.push(c),
            }
        }
        None
    };
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.next() {
            None => break,
            Some('"') => {}
            Some(_) => return Err(bad()),
        }
        let key = string(&mut chars).ok_or_else(bad)?;
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ':') {
            chars.next();
        }
        let value = if chars.peek() == Some(&'"') {
            chars.next();
            string(&mut chars).ok_or_else(bad)?
        } else {
            let mut v = String::new();
            while chars.peek().is_some_and(|c| *c != ',' && *c != '}') {
                v.push(chars.next().unwrap());
            }
            v.trim().to_string()
        };
        out.push((key, value));
    }
    Ok(out)
}

/// "YYYY-MM-DD" to seconds since the Unix epoch (midnight UTC).
fn parse_date(text: &str) -> Option<u64> {
    let mut parts = text.trim().splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // days from civil (Howard Hinnant)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400).ok()
}

// ---- Starting ------------------------------------------------------------------

/// Make the machine for the app at `bundle_path` (an .ipa), with `settings`.
/// Nothing runs until the first frame.
pub fn start(bundle_path: &str, settings: &Settings) -> Result<(), String> {
    touchHLE::time::set_epoch(std::time::Duration::from_secs(settings.start_date));
    touchHLE::time::set_cpu_hz(settings.cpu_hz);
    chimera::check_bundle(bundle_path)?;
    let mut args = vec!["touchHLE".to_string(), bundle_path.to_string()];
    args.extend(settings.options.iter().cloned());
    chimera::boot(args);
    let c = core();
    c.audio = vec![0; AUDIO_PAIRS_PER_FRAME * 2];
    c.running = true;
    Ok(())
}

fn read_mount(name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(name).map_err(|e| format!("{name}: {e}"))
}

#[no_mangle]
pub extern "C" fn Init() -> i32 {
    let settings = match read_mount("settings")
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .and_then(|s| Settings::parse(&s))
    {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    // The engine mounts the game under its own name too, and says which.
    let name = match read_mount("rom.name") {
        Ok(n) => String::from_utf8_lossy(&n).trim().to_string(),
        Err(e) => return fail(e),
    };
    if !name.to_ascii_lowercase().ends_with(".ipa") {
        return fail(format!("{name} is not an .ipa file: this core runs iPhone OS apps packaged as .ipa"));
    }
    match start(&name, &settings) {
        Ok(()) => 1,
        Err(e) => fail(e),
    }
}

#[no_mangle]
pub extern "C" fn GetLoadError() -> *const u8 {
    let e = &core().load_error;
    if e.is_empty() {
        b"\0".as_ptr()
    } else {
        e.as_ptr()
    }
}

// ---- A frame ---------------------------------------------------------------------

fn apply_input() {
    let c = core();
    let (_, w, h) = chimera::video();
    let (w, h) = if w == 0 { (320, 480) } else { (w, h) };
    for finger in 0..chimera::MAX_FINGERS {
        let down = c.buttons & (1 << finger) != 0;
        let ax = c.axes[finger * 2].clamp(0, 65535) as f32;
        let ay = c.axes[finger * 2 + 1].clamp(0, 65535) as f32;
        // pixel = axis * size / 65536, on the picture as it is now
        let at = (ax * w as f32 / 65536.0, ay * h as f32 / 65536.0);
        chimera::set_touch(finger, if down { Some(at) } else { None });
    }
    chimera::set_tilt(
        c.axes[AXIS_TILT_X] as f32 / 32767.0,
        c.axes[AXIS_TILT_Y] as f32 / 32767.0,
    );
}

#[no_mangle]
pub extern "C" fn SetButton(index: i32, state: i32) {
    if (0..BUTTONS.len() as i32).contains(&index) {
        let bit = 1u64 << index;
        let c = core();
        c.buttons = if state != 0 { c.buttons | bit } else { c.buttons & !bit };
    }
}

#[no_mangle]
pub extern "C" fn SetAxis(index: i32, value: i32) {
    if let Some(axis) = core().axes.get_mut(index as usize) {
        *axis = value;
    }
}

#[no_mangle]
pub extern "C" fn FrameAdvance(buttons: u64) {
    let c = core();
    c.buttons = buttons;
    if !c.running {
        return;
    }
    apply_input();
    if let Some(result) = chimera::run_frame() {
        c.running = false;
        match result {
            Ok(()) => eprintln!("touchhle core: the app exited"),
            Err(e) => eprintln!("touchhle core: the app stopped: {e}"),
        }
    }
}

#[no_mangle]
pub extern "C" fn IsRunning() -> i32 {
    core().running as i32
}

#[no_mangle]
pub extern "C" fn GetFrameCount() -> u64 {
    chimera::frame_count()
}

/// Machine time, nanoseconds: what the gate compares between flavors before
/// it compares pictures.
#[no_mangle]
pub extern "C" fn GetMachineTimeNs() -> u64 {
    touchHLE::time::now_ns()
}

// ---- What the frame produced -----------------------------------------------------

#[no_mangle]
pub extern "C" fn GetVideoBgra() -> *const u8 {
    let (pixels, _, _) = chimera::video();
    if pixels.is_empty() {
        BLACK.as_ptr()
    } else {
        pixels.as_ptr()
    }
}
/// Before the first picture: a black iPhone screen.
static BLACK: [u8; 320 * 480 * 4] = [0; 320 * 480 * 4];

#[no_mangle]
pub extern "C" fn GetVideoWidth() -> i32 {
    match chimera::video() {
        (_, 0, _) => 320,
        (_, w, _) => w as i32,
    }
}

#[no_mangle]
pub extern "C" fn GetVideoHeight() -> i32 {
    match chimera::video() {
        (_, _, 0) => 480,
        (_, _, h) => h as i32,
    }
}

#[no_mangle]
pub extern "C" fn GetVsyncNumerator() -> i32 {
    chimera::FRAMES_PER_SECOND as i32
}

#[no_mangle]
pub extern "C" fn GetVsyncDenominator() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn GetAudio() -> *const i16 {
    core().audio.as_ptr()
}

#[no_mangle]
pub extern "C" fn GetAudioSampleCount() -> i32 {
    AUDIO_PAIRS_PER_FRAME as i32
}
