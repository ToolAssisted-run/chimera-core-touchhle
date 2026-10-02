//! run-native: the touchHLE core's native reference.
//!
//! Runs an app the way the engine runs core.wbx - settings, then a frame at a
//! time with that frame's input - but on the host, where a debugger works.
//!
//! usage: run-native <app.ipa> [options]
//!   --frames N              frames to run (default 600)
//!   --setting KEY=VALUE     a setting, as waterbox.config names it (repeatable)
//!   --touch F:FINGER:X:Y    from frame F, finger 1 or 2 touches X,Y (0..65535)
//!   --release F:FINGER      from frame F, that finger is lifted
//!   --tilt F:X:Y            from frame F, tilt (-32768..32767 each)
//!   --screenshot F=PATH     write frame F's picture as a TGA
//!   --digest-every N        print machine time and a picture hash every N frames
//!
//! At the end it prints one line the gate compares between flavors:
//!   frames=N ticks=I time_ns=T video=<hash of the last picture> WxH
//!   audio=<hash of every frame's sound> running=<0|1>

use std::collections::BTreeMap;
use touchhle_guest as core;

fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn audio() -> &'static [i16] {
    let n = core::GetAudioSampleCount() as usize * 2;
    unsafe { std::slice::from_raw_parts(core::GetAudio(), n) }
}

fn video() -> (&'static [u8], usize, usize) {
    let w = core::GetVideoWidth() as usize;
    let h = core::GetVideoHeight() as usize;
    let p = core::GetVideoBgra();
    (unsafe { std::slice::from_raw_parts(p, w * h * 4) }, w, h)
}

fn write_tga(path: &str, bgra: &[u8], w: usize, h: usize) {
    let mut out = vec![0u8; 18];
    out[2] = 2; // uncompressed true colour
    out[12..14].copy_from_slice(&(w as u16).to_le_bytes());
    out[14..16].copy_from_slice(&(h as u16).to_le_bytes());
    out[16] = 32;
    out[17] = 0x28; // top-left origin, 8 alpha bits
    out.extend_from_slice(bgra);
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
}

enum Change {
    Touch(usize, i32, i32),
    Release(usize),
    Tilt(i32, i32),
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut app = None;
    let mut frames: u64 = 600;
    let mut settings: BTreeMap<String, String> = BTreeMap::new();
    let mut changes: BTreeMap<u64, Vec<Change>> = BTreeMap::new();
    let mut shots: BTreeMap<u64, String> = BTreeMap::new();
    let mut digest_every: u64 = 0;
    let num = |s: &str| -> i64 { s.parse().unwrap_or_else(|_| panic!("not a number: {s}")) };
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--frames" => frames = num(&value()) as u64,
            "--setting" => {
                let v = value();
                let (k, v) = v.split_once('=').expect("--setting KEY=VALUE");
                settings.insert(k.to_string(), v.to_string());
            }
            "--touch" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 4, "--touch F:FINGER:X:Y");
                changes.entry(num(p[0]) as u64).or_default().push(Change::Touch(
                    num(p[1]) as usize - 1,
                    num(p[2]) as i32,
                    num(p[3]) as i32,
                ));
            }
            "--release" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 2, "--release F:FINGER");
                changes.entry(num(p[0]) as u64).or_default().push(Change::Release(num(p[1]) as usize - 1));
            }
            "--tilt" => {
                let v = value();
                let p: Vec<&str> = v.split(':').collect();
                assert!(p.len() == 3, "--tilt F:X:Y");
                changes
                    .entry(num(p[0]) as u64)
                    .or_default()
                    .push(Change::Tilt(num(p[1]) as i32, num(p[2]) as i32));
            }
            "--screenshot" => {
                let v = value();
                let (f, p) = v.split_once('=').expect("--screenshot F=PATH");
                shots.insert(num(f) as u64, p.to_string());
            }
            "--digest-every" => digest_every = num(&value()) as u64,
            _ if a.starts_with("--") => panic!("unknown option {a}"),
            _ => app = Some(a),
        }
    }
    let app = app.expect("usage: run-native <app.ipa> [options]");

    // the settings channel, as the engine composes it: a flat JSON object
    let json = format!(
        "{{{}}}",
        settings
            .iter()
            .map(|(k, v)| format!("\"{k}\":\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",")
    );
    let settings = core::Settings::parse(&json).unwrap_or_else(|e| {
        eprintln!("run-native: {e}");
        std::process::exit(2)
    });
    if let Err(e) = core::start(&app, &settings) {
        eprintln!("run-native: {e}");
        std::process::exit(1);
    }

    let mut buttons: u64 = 0;
    // every frame's sound, hashed in order; the loudest sample since the
    // last digest line
    let mut audio_hash: u64 = 0xcbf29ce484222325;
    let mut peak: i32 = 0;
    for frame in 0..frames {
        for change in changes.get(&frame).map(|v| v.as_slice()).unwrap_or(&[]) {
            match *change {
                Change::Touch(finger, x, y) => {
                    buttons |= 1 << finger;
                    core::SetAxis((finger * 2) as i32, x);
                    core::SetAxis((finger * 2 + 1) as i32, y);
                }
                Change::Release(finger) => buttons &= !(1 << finger),
                Change::Tilt(x, y) => {
                    core::SetAxis(4, x);
                    core::SetAxis(5, y);
                }
            }
        }
        core::FrameAdvance(buttons);
        for s in audio() {
            for b in s.to_le_bytes() {
                audio_hash ^= b as u64;
                audio_hash = audio_hash.wrapping_mul(0x100000001b3);
            }
            peak = peak.max((*s as i32).abs());
        }
        let (pixels, w, h) = video();
        if let Some(path) = shots.get(&frame) {
            write_tga(path, pixels, w, h);
        }
        if digest_every > 0 && (frame + 1) % digest_every == 0 {
            println!(
                "frame {} ticks={} time_ns={} video={:016x} {}x{} peak={}",
                frame + 1,
                core::GetExecutedTicks(),
                core::GetMachineTimeNs(),
                fnv64(pixels),
                w,
                h,
                peak
            );
            peak = 0;
        }
        if core::IsRunning() == 0 {
            break;
        }
    }
    let (pixels, w, h) = video();
    println!(
        "frames={} ticks={} time_ns={} video={:016x} {}x{} audio={:016x} running={}",
        core::GetFrameCount(),
        core::GetExecutedTicks(),
        core::GetMachineTimeNs(),
        fnv64(pixels),
        w,
        h,
        audio_hash,
        core::IsRunning()
    );
}
