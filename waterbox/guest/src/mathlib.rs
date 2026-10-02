//! The C library's maths, from the `libm` crate (a Rust port of musl's), in
//! BOTH builds of the core.
//!
//! touchHLE implements an app's sinf/powf/... with Rust's f32 methods, and
//! Mesa (softpipe's shading) and OpenAL Soft call the same names; all of them
//! end up in whatever libm the final link has. In the sandbox that is musl's,
//! natively glibc's, and the two round differently: Terminator Salvation's 3D
//! intro drew 741 pixels one step apart between the flavors while the machine
//! was otherwise identical. Defining the symbols here puts one implementation
//! under every caller in both builds - and makes what a movie depends on the
//! core's own code rather than the libm of whichever sandbox kit or host
//! built it.
//!
//! The list is what the guest link takes from the libm (nm -u over the core's
//! objects); a function missing from it would quietly come from the system
//! again, so waterbox/build-guest.sh checks none does.

#![allow(clippy::missing_safety_doc)]

#[no_mangle]
pub extern "C" fn acosf(x: f32) -> f32 {
    libm::acosf(x)
}

#[no_mangle]
pub extern "C" fn acoshf(x: f32) -> f32 {
    libm::acoshf(x)
}

#[no_mangle]
pub extern "C" fn asinf(x: f32) -> f32 {
    libm::asinf(x)
}

#[no_mangle]
pub extern "C" fn asinhf(x: f32) -> f32 {
    libm::asinhf(x)
}

#[no_mangle]
pub extern "C" fn atanf(x: f32) -> f32 {
    libm::atanf(x)
}

#[no_mangle]
pub extern "C" fn cbrtf(x: f32) -> f32 {
    libm::cbrtf(x)
}

#[no_mangle]
pub extern "C" fn ceilf(x: f32) -> f32 {
    libm::ceilf(x)
}

#[no_mangle]
pub extern "C" fn cosf(x: f32) -> f32 {
    libm::cosf(x)
}

#[no_mangle]
pub extern "C" fn coshf(x: f32) -> f32 {
    libm::coshf(x)
}

#[no_mangle]
pub extern "C" fn exp2f(x: f32) -> f32 {
    libm::exp2f(x)
}

#[no_mangle]
pub extern "C" fn expf(x: f32) -> f32 {
    libm::expf(x)
}

#[no_mangle]
pub extern "C" fn expm1f(x: f32) -> f32 {
    libm::expm1f(x)
}

#[no_mangle]
pub extern "C" fn floorf(x: f32) -> f32 {
    libm::floorf(x)
}

#[no_mangle]
pub extern "C" fn log10f(x: f32) -> f32 {
    libm::log10f(x)
}

#[no_mangle]
pub extern "C" fn log1pf(x: f32) -> f32 {
    libm::log1pf(x)
}

#[no_mangle]
pub extern "C" fn log2f(x: f32) -> f32 {
    libm::log2f(x)
}

#[no_mangle]
pub extern "C" fn logf(x: f32) -> f32 {
    libm::logf(x)
}

#[no_mangle]
pub extern "C" fn roundf(x: f32) -> f32 {
    libm::roundf(x)
}

#[no_mangle]
pub extern "C" fn sinf(x: f32) -> f32 {
    libm::sinf(x)
}

#[no_mangle]
pub extern "C" fn sinhf(x: f32) -> f32 {
    libm::sinhf(x)
}

#[no_mangle]
pub extern "C" fn tanf(x: f32) -> f32 {
    libm::tanf(x)
}

#[no_mangle]
pub extern "C" fn tanhf(x: f32) -> f32 {
    libm::tanhf(x)
}

#[no_mangle]
pub extern "C" fn truncf(x: f32) -> f32 {
    libm::truncf(x)
}

#[no_mangle]
pub extern "C" fn acos(x: f64) -> f64 {
    libm::acos(x)
}

#[no_mangle]
pub extern "C" fn acosh(x: f64) -> f64 {
    libm::acosh(x)
}

#[no_mangle]
pub extern "C" fn asin(x: f64) -> f64 {
    libm::asin(x)
}

#[no_mangle]
pub extern "C" fn asinh(x: f64) -> f64 {
    libm::asinh(x)
}

#[no_mangle]
pub extern "C" fn atan(x: f64) -> f64 {
    libm::atan(x)
}

#[no_mangle]
pub extern "C" fn ceil(x: f64) -> f64 {
    libm::ceil(x)
}

#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    libm::cos(x)
}

#[no_mangle]
pub extern "C" fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

#[no_mangle]
pub extern "C" fn exp(x: f64) -> f64 {
    libm::exp(x)
}

#[no_mangle]
pub extern "C" fn exp2(x: f64) -> f64 {
    libm::exp2(x)
}

#[no_mangle]
pub extern "C" fn expm1(x: f64) -> f64 {
    libm::expm1(x)
}

#[no_mangle]
pub extern "C" fn floor(x: f64) -> f64 {
    libm::floor(x)
}

#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    libm::log(x)
}

#[no_mangle]
pub extern "C" fn log10(x: f64) -> f64 {
    libm::log10(x)
}

#[no_mangle]
pub extern "C" fn log1p(x: f64) -> f64 {
    libm::log1p(x)
}

#[no_mangle]
pub extern "C" fn log2(x: f64) -> f64 {
    libm::log2(x)
}

#[no_mangle]
pub extern "C" fn rint(x: f64) -> f64 {
    libm::rint(x)
}

#[no_mangle]
pub extern "C" fn round(x: f64) -> f64 {
    libm::round(x)
}

#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    libm::sin(x)
}

#[no_mangle]
pub extern "C" fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

#[no_mangle]
pub extern "C" fn tan(x: f64) -> f64 {
    libm::tan(x)
}

#[no_mangle]
pub extern "C" fn tanh(x: f64) -> f64 {
    libm::tanh(x)
}

#[no_mangle]
pub extern "C" fn trunc(x: f64) -> f64 {
    libm::trunc(x)
}

#[no_mangle]
pub extern "C" fn atan2f(x: f32, y: f32) -> f32 {
    libm::atan2f(x, y)
}

#[no_mangle]
pub extern "C" fn fmaxf(x: f32, y: f32) -> f32 {
    libm::fmaxf(x, y)
}

#[no_mangle]
pub extern "C" fn fminf(x: f32, y: f32) -> f32 {
    libm::fminf(x, y)
}

#[no_mangle]
pub extern "C" fn fmodf(x: f32, y: f32) -> f32 {
    libm::fmodf(x, y)
}

#[no_mangle]
pub extern "C" fn hypotf(x: f32, y: f32) -> f32 {
    libm::hypotf(x, y)
}

#[no_mangle]
pub extern "C" fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

#[no_mangle]
pub extern "C" fn atan2(x: f64, y: f64) -> f64 {
    libm::atan2(x, y)
}

#[no_mangle]
pub extern "C" fn fmax(x: f64, y: f64) -> f64 {
    libm::fmax(x, y)
}

#[no_mangle]
pub extern "C" fn fmin(x: f64, y: f64) -> f64 {
    libm::fmin(x, y)
}

#[no_mangle]
pub extern "C" fn fmod(x: f64, y: f64) -> f64 {
    libm::fmod(x, y)
}

#[no_mangle]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

#[no_mangle]
pub extern "C" fn fmaf(x: f32, y: f32, z: f32) -> f32 {
    libm::fmaf(x, y, z)
}
#[no_mangle]
pub extern "C" fn fma(x: f64, y: f64, z: f64) -> f64 {
    libm::fma(x, y, z)
}
#[no_mangle]
pub unsafe extern "C" fn frexpf(x: f32, e: *mut i32) -> f32 {
    let (m, ex) = libm::frexpf(x);
    *e = ex;
    m
}
#[no_mangle]
pub unsafe extern "C" fn frexp(x: f64, e: *mut i32) -> f64 {
    let (m, ex) = libm::frexp(x);
    *e = ex;
    m
}
#[no_mangle]
pub extern "C" fn ldexpf(x: f32, n: i32) -> f32 {
    libm::ldexpf(x, n)
}
#[no_mangle]
pub extern "C" fn ldexp(x: f64, n: i32) -> f64 {
    libm::ldexp(x, n)
}
#[no_mangle]
pub unsafe extern "C" fn modf(x: f64, i: *mut f64) -> f64 {
    let (f, int) = libm::modf(x);
    *i = int;
    f
}
#[no_mangle]
pub unsafe extern "C" fn sincosf(x: f32, s: *mut f32, c: *mut f32) {
    let (sv, cv) = libm::sincosf(x);
    *s = sv;
    *c = cv;
}
#[no_mangle]
pub unsafe extern "C" fn sincos(x: f64, s: *mut f64, c: *mut f64) {
    let (sv, cv) = libm::sincos(x);
    *s = sv;
    *c = cv;
}

/// Every name above, for the link check (build-guest.sh).
pub const NAMES: &[&str] = &[

    "acos",

    "acosf",

    "acosh",

    "acoshf",

    "asin",

    "asinf",

    "asinh",

    "asinhf",

    "atan",

    "atan2",

    "atan2f",

    "atanf",

    "cbrtf",

    "ceil",

    "ceilf",

    "cos",

    "cosf",

    "cosh",

    "coshf",

    "exp",

    "exp2",

    "exp2f",

    "expf",

    "expm1",

    "expm1f",

    "floor",

    "floorf",

    "fma",

    "fmaf",

    "fmax",

    "fmaxf",

    "fmin",

    "fminf",

    "fmod",

    "fmodf",

    "frexp",

    "frexpf",

    "hypotf",

    "ldexp",

    "ldexpf",

    "log",

    "log10",

    "log10f",

    "log1p",

    "log1pf",

    "log2",

    "log2f",

    "logf",

    "modf",

    "pow",

    "powf",

    "rint",

    "round",

    "roundf",

    "sin",

    "sincos",

    "sincosf",

    "sinf",

    "sinh",

    "sinhf",

    "tan",

    "tanf",

    "tanh",

    "tanhf",

    "trunc",

    "truncf",

];
