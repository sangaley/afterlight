//! Where the slow frames go (AFTERLIGHT_STUTTER=<ms>, macOS, a measuring
//! tool).
//!
//! An average frame time hides a stutter: one frame in a hundred taking
//! three times as long is what reads as lag, and a whole-run profile can't
//! see it, because those frames are a sliver of the total. This notes, about
//! once a millisecond through every frame's schedules, which system the main
//! thread is in, and keeps the frames that ran past <ms> apart from the
//! rest. At the end of an AFTERLIGHT_PERF_QUIT run it prints, system by
//! system, the time spent in a slow frame beside the time in a normal one:
//! whatever is much bigger on the left is the stutter.
//!
//! Only the main thread is watched, so run it with AFTERLIGHT_ONE_THREAD=1,
//! which puts every system there.
//!
//! How: a watchdog thread signals the main thread, whose handler takes a
//! backtrace into a fixed buffer (no allocation in the handler); the frame's
//! end turns the addresses into names.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;

/// The threshold in ms, if the tool is on.
pub fn threshold() -> Option<f32> {
    static T: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *T.get_or_init(|| if cfg!(target_os = "macos") { std::env::var("AFTERLIGHT_STUTTER").ok().and_then(|v| v.parse().ok()) } else { None })
}

const DEPTH: usize = 96;
const PER_FRAME: usize = 1024;
static mut STACKS: [[usize; DEPTH]; PER_FRAME] = [[0; DEPTH]; PER_FRAME];
static mut DEPTHS: [u8; PER_FRAME] = [0; PER_FRAME];
static TAKEN: AtomicUsize = AtomicUsize::new(0);
static IN_FRAME: AtomicBool = AtomicBool::new(false);
static MAIN: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "macos")]
mod sys {
    use std::ffi::{c_char, c_int, c_void};
    #[repr(C)]
    pub struct DlInfo {
        pub fname: *const c_char,
        pub fbase: *mut c_void,
        pub sname: *const c_char,
        pub saddr: *mut c_void,
    }
    pub const SIGUSR2: c_int = 31;
    unsafe extern "C" {
        pub fn backtrace(buf: *mut *mut c_void, size: c_int) -> c_int;
        pub fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
        pub fn pthread_self() -> usize;
        pub fn pthread_kill(thread: usize, sig: c_int) -> c_int;
        pub fn signal(sig: c_int, handler: usize) -> usize;
    }
}

#[cfg(target_os = "macos")]
extern "C" fn on_tick(_: std::ffi::c_int) {
    if !IN_FRAME.load(Relaxed) {
        return;
    }
    let i = TAKEN.fetch_add(1, Relaxed);
    if i >= PER_FRAME {
        return;
    }
    // SAFETY: only the main thread's handler writes these, and the main
    // thread reads them only after IN_FRAME is false.
    unsafe {
        let stack = std::ptr::addr_of_mut!(STACKS[i]) as *mut *mut std::ffi::c_void;
        let n = sys::backtrace(stack, DEPTH as std::ffi::c_int);
        *std::ptr::addr_of_mut!(DEPTHS[i]) = n.clamp(0, DEPTH as i32) as u8;
    }
}

#[derive(Default)]
struct Tally {
    names: HashMap<usize, Option<String>>,
    slow: HashMap<String, f32>,
    normal: HashMap<String, f32>,
    /// Inside each owner, where the slow frames were (innermost function).
    slow_inside: HashMap<String, HashMap<String, f32>>,
    slow_frames: usize,
    normal_frames: usize,
    slow_ms: f32,
    normal_ms: f32,
    logged: usize,
}

fn tally() -> &'static Mutex<Tally> {
    static T: std::sync::OnceLock<Mutex<Tally>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

/// The schedules are starting: watch this frame.
pub fn frame_begin() {
    if threshold().is_none() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if MAIN.load(Relaxed) == 0 {
            // SAFETY: plain libc calls; the handler is async-signal-tolerant.
            unsafe {
                MAIN.store(sys::pthread_self(), Relaxed);
                sys::signal(sys::SIGUSR2, on_tick as extern "C" fn(std::ffi::c_int) as usize);
            }
            std::thread::spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_micros(900));
                if IN_FRAME.load(Relaxed) {
                    unsafe { sys::pthread_kill(MAIN.load(Relaxed), sys::SIGUSR2) };
                }
            });
        }
        TAKEN.store(0, Relaxed);
        IN_FRAME.store(true, Relaxed);
    }
}

/// A legacy-mangled Rust symbol as a path, without its hash.
pub fn demangle(sym: &str) -> String {
    let trimmed = sym.trim_start_matches('_');
    let Some(mut rest) = trimmed.strip_prefix("ZN") else { return sym.to_string() };
    let mut parts: Vec<&str> = Vec::new();
    loop {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let Ok(len) = rest[..digits].parse::<usize>() else { break };
        if rest.len() < digits + len {
            break;
        }
        parts.push(&rest[digits..digits + len]);
        rest = &rest[digits + len..];
    }
    if parts.last().is_some_and(|p| p.len() == 17 && p.starts_with('h')) {
        parts.pop();
    }
    let mut out = parts.join("::");
    for (from, to) in [("$LT$", "<"), ("$GT$", ">"), ("$u20$", " "), ("$C$", ","), ("$RF$", "&"), ("$u7b$", "{"), ("$u7d$", "}"), ("..", "::")] {
        out = out.replace(from, to);
    }
    // A segment that starts with a bracket is written with a '_' before it.
    out.replace("::_<", "::<").trim_start_matches('_').to_string()
}

/// A function's name without its closures, for grouping.
fn plain(name: &str) -> String {
    let cut = name.find("::{{closure}}").unwrap_or(name.len());
    let mut s = name[..cut].to_string();
    s.truncate(110);
    s
}

#[cfg(target_os = "macos")]
fn name_of(addr: usize) -> Option<String> {
    let mut info = sys::DlInfo { fname: std::ptr::null(), fbase: std::ptr::null_mut(), sname: std::ptr::null(), saddr: std::ptr::null_mut() };
    // SAFETY: dladdr fills the struct or returns 0.
    unsafe {
        if sys::dladdr(addr as *const std::ffi::c_void, &mut info) == 0 || info.sname.is_null() {
            return None;
        }
        Some(demangle(&std::ffi::CStr::from_ptr(info.sname).to_string_lossy()))
    }
}

#[cfg(not(target_os = "macos"))]
fn name_of(_: usize) -> Option<String> {
    None
}

/// Which system a stack belongs to (the outermost game function, else the
/// outermost engine system), and the innermost function it was in.
fn owner_of(stack: &[Option<String>]) -> (String, String) {
    const WRAPPERS: [&str; 7] = ["bevy_ecs::", "bevy_app::", "bevy_winit::", "bevy_tasks::", "bevy_platform", "bevy_utils", "bevy_render::pipelined_rendering"];
    let named = || stack.iter().rev().flatten();
    let game = |n: &&String| n.contains("afterlight::") && !n.contains("afterlight::main") && !n.contains("afterlight::perf");
    let owner = named()
        .find(|n| n.starts_with("afterlight::") && game(n))
        .or_else(|| named().find(game))
        .or_else(|| named().find(|n| n.starts_with("bevy_") && !WRAPPERS.iter().any(|w| n.starts_with(w))))
        .map_or("(engine: running the schedules)".to_string(), |n| plain(n));
    // Innermost: the first named frame past the signal trampoline.
    let past = stack.iter().position(|n| n.as_deref().is_some_and(|n| n.contains("sigtramp"))).map_or(0, |i| i + 1);
    let inside = stack[past..].iter().flatten().next().map_or("?".to_string(), |n| plain(n));
    (owner, inside)
}

/// The schedules are done: file this frame's notes under slow or normal.
pub fn frame_end(sim_ms: f32, counted: bool) {
    let Some(limit) = threshold() else { return };
    IN_FRAME.store(false, Relaxed);
    let taken = TAKEN.load(Relaxed).min(PER_FRAME);
    if !counted || taken == 0 {
        return;
    }
    let mut t = tally().lock().unwrap();
    let t = &mut *t;
    let slow = sim_ms > limit;
    let each = sim_ms / taken as f32;
    let mut this_frame: HashMap<String, f32> = HashMap::new();
    for i in 0..taken {
        // SAFETY: IN_FRAME is false, so the handler no longer writes.
        let (stack, depth) = unsafe { (*std::ptr::addr_of!(STACKS[i]), *std::ptr::addr_of!(DEPTHS[i]) as usize) };
        let names: Vec<Option<String>> = stack[..depth].iter().map(|a| t.names.entry(*a).or_insert_with(|| name_of(*a)).clone()).collect();
        let (owner, inside) = owner_of(&names);
        if slow {
            *t.slow.entry(owner.clone()).or_default() += each;
            *t.slow_inside.entry(owner.clone()).or_default().entry(inside).or_default() += each;
            *this_frame.entry(owner).or_default() += each;
        } else {
            *t.normal.entry(owner).or_default() += each;
        }
    }
    if slow {
        t.slow_frames += 1;
        t.slow_ms += sim_ms;
        if t.logged < 60 {
            t.logged += 1;
            let mut top: Vec<(String, f32)> = this_frame.into_iter().collect();
            top.sort_by(|a, b| b.1.total_cmp(&a.1));
            let top: Vec<String> = top.iter().take(3).map(|(n, ms)| format!("{} {ms:.1}", n.rsplit("::").next().unwrap_or(n))).collect();
            bevy::log::info!("STUTTER frame sim={sim_ms:.1}ms: {}", top.join(", "));
        }
    } else {
        t.normal_frames += 1;
        t.normal_ms += sim_ms;
    }
}

/// The run is over: slow frames beside normal ones, system by system.
pub fn report() {
    let Some(limit) = threshold() else { return };
    let t = tally().lock().unwrap();
    let (sf, nf) = (t.slow_frames.max(1) as f32, t.normal_frames.max(1) as f32);
    bevy::log::info!(
        "STUTTER SUMMARY over {limit} ms: {} slow frames averaging {:.1} ms, {} normal averaging {:.1} ms",
        t.slow_frames,
        t.slow_ms / sf,
        t.normal_frames,
        t.normal_ms / nf
    );
    let mut rows: Vec<(f32, f32, &String)> = t.slow.iter().map(|(name, ms)| (ms / sf, t.normal.get(name).copied().unwrap_or(0.0) / nf, name)).collect();
    rows.sort_by(|a, b| (b.0 - b.1).total_cmp(&(a.0 - a.1)));
    for (slow, normal, name) in rows.iter().take(25) {
        let mut inside: Vec<(&String, &f32)> = t.slow_inside.get(*name).map(|m| m.iter().collect()).unwrap_or_default();
        inside.sort_by(|a, b| b.1.total_cmp(a.1));
        let inside: Vec<String> = inside.iter().take(3).map(|(n, ms)| format!("{} {:.1}", n.rsplit("::").take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("::"), **ms / sf)).collect();
        bevy::log::info!("STUTTER {slow:6.2} ms slow | {normal:5.2} ms normal | {name}  [{}]", inside.join("; "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mangled_name_reads_as_a_path() {
        assert_eq!(demangle("_ZN10afterlight3vfx9ship_roof9fit_roofs17h0123456789abcdefE"), "afterlight::vfx::ship_roof::fit_roofs");
        assert_eq!(demangle("__ZN4core3ops8function5impls79_$LT$impl$u20$core..ops..function..FnMut$LT$A$GT$$u20$for$u20$$RF$mut$u20$F$GT$8call_mut17h0123456789abcdefE"), "core::ops::function::impls::<impl core::ops::function::FnMut<A> for &mut F>::call_mut");
        assert_eq!(demangle("malloc"), "malloc");
    }

    #[test]
    fn a_stack_belongs_to_its_outermost_game_function() {
        let stack: Vec<Option<String>> = ["on_tick", "_sigtramp", "alloc::vec::Vec<T>::push", "afterlight::vfx::far_art::paint", "afterlight::vfx::far_art::bake_far_art::{{closure}}", "bevy_ecs::schedule::executor::run", "afterlight::main"]
            .iter()
            .map(|s| Some(s.to_string()))
            .collect();
        assert_eq!(owner_of(&stack), ("afterlight::vfx::far_art::bake_far_art".to_string(), "alloc::vec::Vec<T>::push".to_string()));
        let engine: Vec<Option<String>> = ["on_tick", "_sigtramp", "memmove", "bevy_transform::systems::propagate", "bevy_ecs::schedule::run"].iter().map(|s| Some(s.to_string())).collect();
        assert_eq!(owner_of(&engine).0, "bevy_transform::systems::propagate");
    }
}
