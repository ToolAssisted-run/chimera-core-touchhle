//! The touchHLE core: the exports the Chimera engine calls, over touchHLE's
//! frame-at-a-time machine (extern/touchHLE, feature "chimera").
//!
//! The same code is the native reference: waterbox/run-native links this crate
//! for the host and calls these functions in the order the engine does, so the
//! two flavors differ only in where they run.

#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]

use serde_json::Value;
use std::ffi::CString;
use std::io::Read;
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

/// 44.1 kHz stereo at 60 frames a second: OpenAL's mix of every device the
/// app opened, rendered at the end of each frame.
pub use chimera::AUDIO_PAIRS_PER_FRAME;

struct Core {
    /// Why Init refused, NUL-terminated.
    load_error: Vec<u8>,
    buttons: u64,
    axes: [i32; 6],
    running: bool,
    /// The save data listed by the last GetSaveDataFileCount, names
    /// NUL-terminated.
    saves: Vec<(Vec<u8>, Vec<u8>)>,
}
static mut CORE: Core = Core {
    load_error: Vec::new(),
    buttons: 0,
    axes: [32768, 32768, 32768, 32768, 0, 0],
    running: false,
    saves: Vec::new(),
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

/// The settings the engine mounts ("settings", a JSON object, every declared
/// setting present), as touchHLE options and machine parameters.
pub struct Settings {
    /// touchHLE command-line options.
    pub options: Vec<String>,
    /// Seconds since the Unix epoch when the machine starts.
    pub rtc_start: u64,
    /// The CPU clock, Hz.
    pub cpu_hz: u64,
    /// Arguments for the app itself (what follows touchHLE's --args). Not a
    /// setting the package declares: the gate passes TestApp --cli-tests.
    pub app_args: Vec<String>,
}

fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Settings {
    pub fn parse(json: &str) -> Result<Settings, String> {
        let mut s = Settings {
            options: Vec::new(),
            rtc_start: 946_684_800, // 2000-01-01
            cpu_hz: 412_000_000,
            app_args: Vec::new(),
        };
        let root: Value = if json.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(json).map_err(|e| format!("the settings are not JSON: {e}"))?
        };
        let Value::Object(map) = root else {
            return Err("the settings are not a JSON object".to_string());
        };
        for (key, value) in &map {
            let text = as_text(value);
            match key.as_str() {
                "device_family" => match text.as_str() {
                    "auto" => {}
                    "iphone" | "ipad" => s.options.push(format!("--device-family={text}")),
                    _ => return Err(format!("device_family: unknown value {text:?}")),
                },
                "orientation" => match text.as_str() {
                    "app" | "portrait" => {}
                    "upside-down" | "landscape-left" | "landscape-right" => {
                        s.options.push(format!("--{text}"))
                    }
                    _ => return Err(format!("orientation: unknown value {text:?}")),
                },
                "rtc_start" => {
                    s.rtc_start = text
                        .parse::<u64>()
                        .map_err(|_| format!("rtc_start: {text:?} is not a time"))?
                }
                "cpu_mhz" => {
                    let mhz: u64 = text
                        .parse()
                        .ok()
                        .filter(|&m| (1..=10_000).contains(&m))
                        .ok_or_else(|| format!("cpu_mhz: {text:?} is not a clock"))?;
                    s.cpu_hz = mhz * 1_000_000;
                }
                "appArgs" => s.app_args = text.split_whitespace().map(String::from).collect(),
                _ => {} // a setting this build does not know is not an error
            }
        }
        // touchHLE's "app" orientation lets its per-app defaults decide;
        // landscape apps it knows open landscape.
        Ok(s)
    }
}

// ---- Save data ---------------------------------------------------------------

/// The files of a save-data zip (what Export Save Data writes): paths under
/// the app's home, Documents/... and Library/....
pub fn read_save_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a zip: {e}"))?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| format!("{name}: {e}"))?;
        out.push((name, data));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

// ---- Starting ------------------------------------------------------------------

/// Make the machine for the app at `bundle_path` (an .ipa), with `settings`
/// and the app's earlier save data. Nothing runs until the first frame.
pub fn start(
    bundle_path: &str,
    settings: &Settings,
    save_data: Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    touchHLE::time::set_epoch(std::time::Duration::from_secs(settings.rtc_start));
    touchHLE::time::set_cpu_hz(settings.cpu_hz);
    chimera::check_bundle(bundle_path)?;
    chimera::set_save_data(save_data).map_err(|e| format!("the save data: {e}"))?;
    // the app's memory exists from here on, at an address that never moves
    chimera::app_memory();
    let mut args = vec!["touchHLE".to_string(), bundle_path.to_string()];
    args.extend(settings.options.iter().cloned());
    if !settings.app_args.is_empty() {
        args.push("--args".to_string());
        args.extend(settings.app_args.iter().cloned());
    }
    chimera::boot(args);
    core().running = true;
    Ok(())
}

fn read_mount(name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(name).map_err(|e| format!("{name}: {e}"))
}

/// The first file of slot `id` in the project's slot map ("slots": a JSON
/// object of slot id to file names, each file mounted under its own name).
/// None without a project or without that slot.
fn slot_first(id: &str) -> Option<String> {
    let text = std::fs::read("slots").ok()?;
    let map: Value = serde_json::from_slice(&text).ok()?;
    map.get(id)?.get(0)?.as_str().map(String::from)
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
    // The app: a project names it in its "game" slot; a file opened on its
    // own arrives with its name in rom.name (and mounted under it too).
    let name = match slot_first("game") {
        Some(n) => n,
        None => match read_mount("rom.name") {
            Ok(n) => String::from_utf8_lossy(&n).trim().to_string(),
            Err(e) => return fail(format!("no app: {e}")),
        },
    };
    let base = name.rsplit('/').next().unwrap_or(&name).to_string();
    let path = if std::path::Path::new(&base).is_file() {
        base
    } else {
        name
    };
    if !path.to_ascii_lowercase().ends_with(".ipa") {
        return fail(format!(
            "{path} is not an .ipa file: this core runs iPhone OS apps packaged as .ipa"
        ));
    }
    // What the app saved before: the .zip Export Save Data writes.
    let save_data = match slot_first("savedata") {
        None => Vec::new(),
        Some(zip_name) => {
            let zip_base = zip_name.rsplit('/').next().unwrap_or(&zip_name).to_string();
            match read_mount(&zip_base).and_then(|b| read_save_zip(&b)) {
                Ok(files) => files,
                Err(e) => return fail(format!("the save data {zip_base}: {e}")),
            }
        }
    };
    match start(&path, &settings, save_data) {
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

/// Instructions the machine has executed: what the gate compares between
/// flavors first.
#[no_mangle]
pub extern "C" fn GetExecutedTicks() -> u64 {
    touchHLE::time::ticks()
}

/// Machine time, nanoseconds.
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
    let audio = chimera::audio();
    if audio.is_empty() {
        SILENCE.as_ptr()
    } else {
        audio.as_ptr()
    }
}
/// Before the first frame's sound.
static SILENCE: [i16; AUDIO_PAIRS_PER_FRAME * 2] = [0; AUDIO_PAIRS_PER_FRAME * 2];

#[no_mangle]
pub extern "C" fn GetAudioSampleCount() -> i32 {
    AUDIO_PAIRS_PER_FRAME as i32
}

// ---- Save data export (Emulator > Export Save Data...) -------------------------

/// Lists what the app has saved, now: every file under its Documents and
/// Library folders. The frontend writes them as a zip, which the savedata
/// slot takes back.
#[no_mangle]
pub extern "C" fn GetSaveDataFileCount() -> i32 {
    let c = core();
    c.saves = chimera::save_data()
        .into_iter()
        .map(|(name, bytes)| (CString::new(name).unwrap_or_default().into_bytes_with_nul(), bytes))
        .collect();
    c.saves.len() as i32
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileName(i: i32) -> *const u8 {
    match core().saves.get(i as usize) {
        Some((name, _)) => name.as_ptr(),
        None => b"\0".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileSize(i: i32) -> i64 {
    core().saves.get(i as usize).map_or(0, |(_, b)| b.len() as i64)
}

#[no_mangle]
pub extern "C" fn GetSaveDataFileBuffer(i: i32) -> *const u8 {
    match core().saves.get(i as usize) {
        Some((_, b)) => b.as_ptr(),
        None => std::ptr::null(),
    }
}

// ---- Memory (RAM Watch, RAM Search, Hex Editor) -------------------------------

/// One domain: the app's whole 32-bit address space as it sees it - its
/// code, its heap and its threads' stacks at their real addresses, so an
/// address an app disassembly names is the address here. Made in Init,
/// before the machine starts, and never moved.
#[no_mangle]
pub extern "C" fn GetMemoryDomainCount() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainName(i: i32) -> *const u8 {
    match i {
        0 => b"App Memory\0".as_ptr(),
        _ => b"\0".as_ptr(),
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainPtr(i: i32) -> *const u8 {
    match i {
        0 => chimera::app_memory().0,
        _ => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainSize(i: i32) -> i64 {
    match i {
        0 => chimera::app_memory().1 as i64,
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn GetMemoryDomainWritable(i: i32) -> i32 {
    (i == 0) as i32
}
