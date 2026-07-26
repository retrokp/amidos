//! Safe API for AmigaDOS to write CLI programs for Amiga m68k.
//!
//! The programs are written for the experimental bare metal
//! [`m68k-unknown-none-elf`](https://doc.rust-lang.org/rustc/platform-support/m68k-unknown-none-elf.html)
//! target, which doesn't have Rust `std` library. Only Rust `no_std` (`core` and `alloc`)
//! are available.
//!
//! # Examples
//!
//! ```
//! #![no_std]
//! #![no_main]
//! #![feature(asm_experimental_arch)]
//!
//! amidos::startup_code!();
//! amidos::panic_handler_abort!();
//!
//! fn amidos_main(dos: &mut amidos::Dos, args: amidos::MainArgs) -> i32 {
//!     let Some(mut output) = dos.output() else {
//!         return amidos::EXIT_CODE_ERROR; // no output stream: launched from Workbench
//!     };
//!     let _ = output.write_all("Hello Amiga!\n".as_bytes());
//!     amidos::EXIT_CODE_OK
//! }
//! ```
//!
//! # Startup code
//!
//! The program startup code typically preprocesses CLI and Workbench arguments.
//! For that, use the [`startup_code!()`] macro, which then calls
//! `amidos_main()` with a reference to the Dos library and arguments.
//! Exit the program by returning an exit code from the main function. Note that on Amiga,
//! the exit code -1 has a special meaning and and prints "Unknown command" when returned to CLI.
//!
//! # Stack
//!
//! For Amiga CLI programs, the default stack size is about 4000 bytes. It can be changed by
//! running the `STACK` CLI command before running CLI programs.
//!
//! For Workbench programs, the stack size can be set in the program's `.info` icon file.
//!
//! Some system functions may consume 1 kilobytes of stack or even more, so it's a good idea to
//! leave some stack for them.
//!
//! The start up code can notice stack overflows when `stack-checker` feature flag is enabled.
//! When a program starts, a canary value is stored at the bottom of the stack. If the same value
//! isn't found during program exit, an error is printed to CLI: "*** ERROR: stack overflow".
//! Workbench programs show a recoverable alert "35BBDEAD". This makes noticing
//! stack overflows easier during development. Note that this is not a security feature.
//!
//! # Heap
//!
//! This crate doesn't currently use heap memory or `core::alloc`, but this may change
//! in the future.
//!
//! The [`global_allocmem!()`] macro can be used to implement a global allocator, which
//! uses the system functions `AllocMem()` and `FreeMem()` to manage heap allocations
//! so that the Rust `alloc` crate can be used.
//!
//! # Strings and text encoding
//!
//! Text is handled as nul-terminated `CStr`, `CString` and `c".."` strings. They are assumed to be
//! encoded in [ECMA94](https://en.wikipedia.org/wiki/ISO/IEC_8859-1#History) (ISO-8859-1 / Latin1).
//! Therefore, `CStr` and `CString` should *not* be converted from/to Rust `&str` or `String`,
//! which use UTF-8 encoding.
//!
//! Non-ASCII character can be entered as escape sequences in string literals, like this:
//! `c"Na\xEFve"`.
//!
//! # Panics
//!
//! This crate tries to avoid panicking, but user programs may panic for various reasons.
//!
//! Two [panic strategies](https://doc.rust-lang.org/reference/panic.html) are possible:
//! `abort` and `immediate-abort`. The third strategy (`unwind`) isn't possible in no_std programs.
//! If a program panics, resources and memory are not freed because no stack unwinding happens.
//!
//! This crate provides macros to automatically implement panic handlers:
//!  - [`panic_handler_abort!()`]: for `abort` with verbose printing
//!  - [`panic_handler_immediate_abort!()`]: for `immediate-abort` with some printing
//!  - [`panic_handler_immediate_abort_minimal!()`]: for `immediate-abort` without printing
//!
//! # Crate features
//!
//! By default, only Kickstart 1.0 functions and methods are enabled. To enable features
//! for later Kickstart versions, use feature flags. Later Kickstart versions enable all
//! earlier Kickstart versions.
//!
//! - `v36`: Enables features for Kickstart 2.0 (v36).
//! - `v37`: Enables features for Kickstart 2.04 (v37) and all earlier versions (v36).
//!
//! Functions and methods called on a system without the required Kickstart version will return
//! an `UnsupportedLibraryVersion` error.
//!
//! The `stack-checker` feature flag enables the stack overflow checker.
//!

#![no_std]
#![feature(asm_experimental_arch)]

// this experimental attribute enables "Available on crate feature" doc comments in docs.rs
#![cfg_attr(docsrs, feature(doc_cfg))]

mod dos;
mod error;

use core::arch::asm;

use core::fmt::Debug;
use core::ffi::CStr;

pub use dos::{DateStamp, Dos, SeekFrom, File, FileLock, Examine, FileInfoBlock, FileType,
    FileLockMode, ProtectionBits};
pub use error::Error;

/// The program OK exit code (0).
pub const EXIT_CODE_OK: i32 = 0;
/// The program WARN exit code (5).
pub const EXIT_CODE_WARN: i32 = 5;
/// The program ERROR exit code (10). Something went wrong.
pub const EXIT_CODE_ERROR: i32 = 10;
/// The program FAIL exit code (20). Complete or severe failure.
pub const EXIT_CODE_FAIL: i32 = 20;

/// Macro to insert program version information to the executable.
///
/// This macro should be used only once per executable. It inserts a
/// ["$VER" version string](https://jvaltane.kapsi.fi/amiga/howtocode/generalguidelines.html#version)
/// to the executable.
///
/// Note: currently, only ascii characters are possible in the program name.
///
/// # Examples
///
/// Hardcoded name and version:
/// ```
/// amidos::version_string!("my-program", "1.213");
/// ```
/// Inspecting the program on Workbench 3.1 (`VERSION` command 40.1) shows:
/// ```custom
/// > VERSION my-program FULL
/// my-program 1.213
/// ```
///
/// Using version from Cargo.toml, only major.minor (no patch number):
/// ```
/// amidos::version_string!("my-program",
///     concat!(env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR")));
/// ```
///
/// Using name and full version from Cargo.toml:
/// ```
/// amidos::version_string!(env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
/// ```
///
/// For a full list of useful name and version environment variables, see the
/// [Cargo Book](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates).
#[macro_export]
macro_rules! version_string {
    // the version command on Amiga 3.1 has Y2K bug and it can't show dates after 1.1.2000,
    // so date and comment parameters are currently commented out
    ($name: expr, $version_revision: expr) => {
    //($name: literal, $version_revision: literal, $date: literal, $comment: literal) => {
        #[used]
        static _VERSION_STRING: &core::ffi::CStr = unsafe {
            core::ffi::CStr::from_bytes_with_nul_unchecked(
                concat!("$VER: ", $name, " ", $version_revision,
                    "\0").as_bytes()
                    //" (", $date, ") ", $comment ,"\0").as_bytes()
            )
        };
    }
}

/// Startup code macro, generates the `_start` entry point, which calls `amidos_main()`.
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! startup_code {
    () => {

// _start written in assembly code
// it goes to the .init section so that it is the first code block in the executable,
// which is required on Amiga
#[unsafe(no_mangle)]
#[unsafe(link_section = ".init")]
#[unsafe(naked)]
pub extern "C" fn _start() {
    core::arch::naked_asm!(
        // calculate stack upper addr
        "lea    8(%sp), %a1",
        // get stack size (CLI and Workbench both store stack size to stack)
        "move.l 4(%sp), %d1",
        // push arguments to stack
        "move.l #{amimain}, -(%sp)", // amidos_main_fn
        "move.l %d0, -(%sp)",   // cli_arg_len
        "move.l %a0, -(%sp)",   // cli_arg_ptr
        "move.l %d1, -(%sp)",   // stack_size
        "move.l %a1, -(%sp)",   // stack_upper_addr

        // call _start_rs, it returns the exit code in %d0
        "jsr {startrs}",

        // pop arguments from stack
        "lea    20(%sp), %sp",

        // exit the program
        "rts",
        startrs = sym amidos::_start_rs,
        amimain = sym crate::amidos_main,
    );
}
}
}

/// Stack size in bytes.
static mut _INTERNAL_STACK_SIZE: usize = 0;
/// Stack upper address.
static mut _INTERNAL_UPPER_STACK_POINTER: usize = 0;

/// Stack canary value "beep bird".
const STACK_CANARY: u32 = 0xbeeb_b12d;

/// Stack overflow error message.
const STACK_OVERFLOW_MSG: &[u8; 26] = b"*** ERROR: stack overflow\n";

/// Internal start function.
#[doc(hidden)]
pub fn _start_rs(
    stack_upper_addr: usize,
    stack_size: usize,
    cli_arg_ptr: *const u8,
    cli_arg_len: u32,
    amidos_main_fn: fn(&mut dos::Dos, MainArgs) -> i32,
) -> i32 {
    unsafe {
        _INTERNAL_STACK_SIZE = stack_size;
        let abs_exec_lib = amiga_sys::abs_exec_library();
        let process =
            amiga_sys::FindTask(abs_exec_lib, core::ptr::null()) as *mut amiga_sys::Process;
        let Ok(mut dos) = crate::dos::Dos::new() else {
            // just return, no call to Alert(AG_OpenLib | AO_DOSLib), because it reboots on KS 1.2
            return EXIT_CODE_FAIL;
        };
        if (*process).pr_CLI != 0 {
            // started from CLI
            let cli_ptr = ((*process).pr_CLI * 4) as *const amiga_sys::CommandLineInterface;
            let cmd_name_buf = ((*cli_ptr).cli_CommandName * 4) as *const u8;
            // alloc mem for command name and arguments (two null-terminated strings)
            let cmd_name_len = *cmd_name_buf;
            let cmdarg_mem_size = u32::from(cmd_name_len) + 1 + cli_arg_len + 1;
            let cmdarg_mem_ptr = amiga_sys::AllocMem(
                abs_exec_lib,
                cmdarg_mem_size,
                amiga_sys::MEMF_PUBLIC | amiga_sys::MEMF_CLEAR,
            );
            if cmdarg_mem_ptr.is_null() {
                // just return, no call to Alert(AG_NoMemory), because it reboots on KS 1.2
                return EXIT_CODE_FAIL;
            }
            // TODO: change not to copy if task name or arguments can't change during main execution
            // copy command name and arguments to guarantee that they are not changed while main()
            // is running (the task name or arguments may change during program execution?)
            // TODO: core::ptr::copy() crashes
            //core::ptr::copy(cmd_name_buf.wrapping_add(1), cmdarg_mem_ptr as *mut u8, cmd_name_len as usize);
            copy(
                cmd_name_buf.wrapping_add(1),
                cmdarg_mem_ptr as *mut u8,
                cmd_name_len as usize,
            );
            let arg_cstr_ptr = (cmdarg_mem_ptr as *mut u8).wrapping_add(cmd_name_len as usize + 1);
            copy(cli_arg_ptr, arg_cstr_ptr, cli_arg_len as usize);
            // TODO: if name or args can contain interior nul chars, then change &CStr to &[u8]
            // (from_bytes_with_nul_unchecked does not work correctly if its param has interior nuls)
            // TODO: compiler bugs.. change to use safer from_ptr() when the compiler allows it
            let command_slice =
                core::slice::from_raw_parts(cmdarg_mem_ptr as *const u8, cmd_name_len as usize + 1);
            let arguments_slice =
                core::slice::from_raw_parts(arg_cstr_ptr, cli_arg_len as usize + 1);
            let command_name = CStr::from_bytes_with_nul_unchecked(command_slice);
            let arguments = CStr::from_bytes_with_nul_unchecked(arguments_slice);
            //let command_name = CStr::from_ptr(cmdarg_mem_ptr as *const core::ffi::c_char);
            //let arguments = CStr::from_ptr(arg_cstr_ptr as *const core::ffi::c_char);
            // CliArgs lifetime is 'static from the main() function point of view
            let args = MainArgs::Cli(CliArgs {
                command_name,
                arguments,
            });
            // use upper stack ptr taken from stack, because the task structure has
            // the system CLI process stack ptr
            _INTERNAL_UPPER_STACK_POINTER = stack_upper_addr;

            let mut sbottom = core::ptr::null_mut();
            if cfg!(feature = "stack-checker") {
                // stack overflow detection: put the canary to the bottom of the stack
                sbottom = (_INTERNAL_UPPER_STACK_POINTER - _INTERNAL_STACK_SIZE) as *mut u32;
                *sbottom = STACK_CANARY;
            }
            // call main
            let exit_code = amidos_main_fn(&mut dos, args);
            amiga_sys::FreeMem(abs_exec_lib, cmdarg_mem_ptr, cmdarg_mem_size);

            if cfg!(feature = "stack-checker") {
                // check canary is still alive
                if *sbottom != STACK_CANARY {
                    if let Some(mut out) = dos.output() {
                        let _ = out.write_all(STACK_OVERFLOW_MSG);
                    }
                    return 20;
                }
            }
            exit_code
        } else {
            // started from workbench
            let msgport = (&mut (*process).pr_MsgPort) as *mut amiga_sys::MsgPort;
            amiga_sys::WaitPort(abs_exec_lib, msgport);
            let msg = amiga_sys::GetMsg(abs_exec_lib, msgport);
            // TODO: set current dir to be the same as the first arguments file lock? and
            // store the old lock so it can be restored before exit?
            // TODO: set sm_ToolWindow for std io?
            // TODO: set the console task so that Open("*", mode) will work?
            let wbmsg = msg as *const amiga_sys::WBStartup;
            let wb_arg_cnt = usize_from_i32((*wbmsg).sm_NumArgs);
            let args = MainArgs::Workbench(WbArgs {
                dos_lib: dos.dos_lib,
                wb_arg_ptr: (*wbmsg).sm_ArgList as *const amiga_sys::WBArg,
                wb_arg_cnt,
            });
            // use upper stack ptr taken from task structure, this matches stack_upper_addr but
            // its safer to use the system provided address (for future compatibility)
            _INTERNAL_UPPER_STACK_POINTER = (*process).pr_Task.tc_SPUpper as usize;

            let mut sbottom = core::ptr::null_mut();
            if cfg!(feature = "stack-checker") {
                // stack overflow detection: put the canary to the bottom of the stack
                sbottom = (_INTERNAL_UPPER_STACK_POINTER - _INTERNAL_STACK_SIZE) as *mut u32;
                *sbottom = STACK_CANARY;
            }
            // call main
            let exit_code = amidos_main_fn(&mut dos, args);
            amiga_sys::Forbid(abs_exec_lib);
            amiga_sys::ReplyMsg(abs_exec_lib, msg);

            if cfg!(feature = "stack-checker") {
                // check canary is still alive
                if *sbottom != STACK_CANARY {
                    // note: this is nasty on Kickstart<2.0 because the system boots after the alert
                    amiga_sys::Alert(abs_exec_lib, amiga_sys::AN_Unknown | 0xbb_dead);
                    return 20;
                }
            }
            exit_code
        }
    }
}

/// Panic handler for the default panic strategy (abort).
///
/// Prints out "*** PANIC" and details about the source of the panic to CLI and
/// exits the program with the error code 20.
/// This panic handler creates large executables.
///
/// `panic = "abort"` is the default panic strategy for no_std programs, so there's no need to
/// modify Cargo.toml to use this macro.
#[macro_export]
macro_rules! panic_handler_abort {
    () => {
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // exit and never come back
    amidos::_panic_exit_abort(info);
    // not reached, but this is needed so that the compiler thinks that this fn never returns
    loop {}
}
    };
}

/// Panic handler aborting immediately.
///
/// Prints out "*** PANIC" to CLI and exits the program with the error code 20.
/// This is a fairly small panic handler.
///
/// To use immediate abort, Cargo.toml must include `cargo-features = ["panic-immediate-abort"]` and
/// `panic = "immediate-abort"`.
#[macro_export]
macro_rules! panic_handler_immediate_abort {
    () => {
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn abort() -> ! {
    // exit and never come back
    amidos::_panic_exit_immediate_abort();
    // not reached, but this is needed so that the compiler thinks that this fn never returns
    loop {}
}
    };
}

/// Panic handler aborting immediately, minimal implementation returning only the error exit code.
///
/// Prints nothing and exits the program with the error code 20.
/// This is the smallest panic handler.
///
/// To use immediate abort, Cargo.toml must include `cargo-features = ["panic-immediate-abort"]` and
/// `panic = "immediate-abort"`.
#[macro_export]
macro_rules! panic_handler_immediate_abort_minimal {
    () => {
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn abort() -> ! {
    // exit and never come back
    amidos::_panic_exit_immediate_abort_minimal(20);
    // not reached, but this is needed so that the compiler thinks that this fn never returns
    loop {}
}
    };
}

/// Internal forced exit of program for `abort` strategy.
#[doc(hidden)]
pub fn _panic_exit_abort(info: &core::panic::PanicInfo) {
    unsafe {
        // open dos.library to print out an error message
        let execlib = amiga_sys::abs_exec_library();
        let doslib = amiga_sys::OpenLibrary(execlib, b"dos.library\0".as_ptr(), 0);
        if !doslib.is_null() {
            let output = amiga_sys::Output(doslib);
            if output != 0 {
                amiga_sys::Write(doslib, output,
                    b"*** PANIC".as_ptr() as *const core::ffi::c_void, 9);
                if let Some(loc) = info.location() {
                    amiga_sys::Write(doslib, output, b" at ".as_ptr() as *const core::ffi::c_void, 4);
                    // note: panic msg is UTF-8 but here we print it out as ISO-8859-1
                    let sourcefile = loc.file();
                    amiga_sys::Write(doslib, output, sourcefile.as_ptr() as *const core::ffi::c_void,
                        i32_from_usize(sourcefile.len()));
                    // TODO: add printing line and column numbers when printing numbers is possible
                    //amiga_sys::Write(doslib, output, b":".as_ptr() as *const core::ffi::c_void, 2);
                    //TodoWriteNumber(doslib, output, loc.line());
                    //amiga_sys::Write(doslib, output, b":".as_ptr() as *const core::ffi::c_void, 2);
                    //TodoWriteNumber(doslib, output, loc.column());
                }
                if let Some(msg) = info.message().as_str() {
                    amiga_sys::Write(doslib, output, b": ".as_ptr() as *const core::ffi::c_void, 2);
                    // note: panic msg is UTF-8 but here we print it out as ISO-8859-1
                    amiga_sys::Write(doslib, output, msg.as_ptr() as *const core::ffi::c_void,
                        i32_from_usize(msg.len()));
                }
                amiga_sys::Write(doslib, output, b"\n".as_ptr() as *const core::ffi::c_void, 1);
            } else {
                // TODO: what to do if output is 0 (program launched from workbench)?
            }
            amiga_sys::CloseLibrary(execlib, doslib);
        }
        // exit and never come back
        _panic_exit_immediate_abort_minimal(20);
    }
}

/// Internal forced exit of program for `immediate-abort` strategy.
#[doc(hidden)]
pub fn _panic_exit_immediate_abort() {
    unsafe {
        // open dos.library to print out an error message
        let execlib = amiga_sys::abs_exec_library();
        let doslib = amiga_sys::OpenLibrary(execlib, b"dos.library\0".as_ptr(), 0);
        if !doslib.is_null() {
            let output = amiga_sys::Output(doslib);
            if output != 0 {
                amiga_sys::Write(doslib, output,
                    b"*** PANIC\n".as_ptr() as *const core::ffi::c_void, 10);
            } else {
                // TODO: what to do if output is 0 (program launched from workbench)?
            }
            amiga_sys::CloseLibrary(execlib, doslib);
        }
        // exit and never come back
        _panic_exit_immediate_abort_minimal(20);
    }
}

/// Internal forced exit of program. Moves stack pointer to initial location and
/// returns to the system. Resources and memory are *not* freed.
#[doc(hidden)]
pub fn _panic_exit_immediate_abort_minimal(exit_code: i32) {
    unsafe {
        asm!(
            // restore stack pointer (not dropping objects in the stack)
            "move.l {upper_sp}, %a1",
            "lea    (-8, %a1), %sp",
            // return to the system (does not return to the function caller!)
            "rts",
            upper_sp = sym crate::_INTERNAL_UPPER_STACK_POINTER,
            in("d0") exit_code,
        );
    }
}

/// Simple heap memory global allocator.
///
/// This allocator directly calls the system `AllocMem()` and `FreeMem()`. The maximum alignment
/// is 4 bytes.
///
/// Note that using this allocator for small allocations (CString, Vec, ..) may
/// fragment the system memory. It's better to use a pooled memory allocator for small allocations.
///
/// Ensure that `.cargo/config.toml` contains `build-std = ["panic_abort", "core", "alloc"]`
/// or that the cargo build command includes building with "alloc".
///
/// # Examples
///
/// ```
/// extern crate alloc;
/// amidos::global_allocmem!();
/// ```
#[macro_export]
macro_rules! global_allocmem {
    () => {
struct AmidosGlobalAllocator;

unsafe impl core::alloc::GlobalAlloc for AmidosGlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {

        // Amiga AllocMem() always allocates with 4 byte alignment
        const MAX_SUPPORTED_ALIGN: usize = 4;

        let align = layout.align();
        if align > MAX_SUPPORTED_ALIGN {
            return core::ptr::null_mut();
        }
        unsafe {
            // call AllocMem() to allocate memory, not clearing the memory,
            // Rust docs say "The allocated block of memory may or may not be initialized."
            let ptr = amidos::_alloc_mem(layout.size());
            // the returned ptr may be null, which is ok: that's how alloc() is supposed to
            // return errors
            ptr as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe {
            // Call FreeMem() to free memory
            amidos::_free_mem(ptr, layout.size());
        }
    }
}
#[global_allocator]
static AMIDOS_GLOBAL_ALLOCATOR: AmidosGlobalAllocator = AmidosGlobalAllocator;

    };
}

#[doc(hidden)]
pub fn _alloc_mem(size: usize) -> *mut core::ffi::c_void {
    unsafe {
        amiga_sys::AllocMem(amiga_sys::abs_exec_library(), u32_from_usize(size), 0)
    }
}

#[doc(hidden)]
pub fn _free_mem(ptr: *mut u8, size: usize) {
    unsafe {
        amiga_sys::FreeMem(amiga_sys::abs_exec_library(), ptr as *mut core::ffi::c_void,
            u32_from_usize(size));
    }
}

// temporary solution to copy memory until compiler starts working properly..
fn copy(src: *const u8, dest: *mut u8, size: usize) {
    if !src.is_null() && !dest.is_null() {
        let mut s = src;
        let mut d = dest;
        for _ in 0..size {
            unsafe {
                *d = *s;
            }
            s = s.wrapping_add(1);
            d = d.wrapping_add(1);
        }
    }
}
/// CLI or Workbench arguments.
pub enum MainArgs {
    Cli(CliArgs),
    Workbench(WbArgs),
}

/// CLI arguments.
///
/// Part of [`MainArgs`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Default)]
pub struct CliArgs {
    /// CLI command name.
    pub command_name: &'static CStr,
    /// All command arguments as one string (doesn't include the command name). The trailing
    /// newline '\n' (0x0a) is included.
    pub arguments: &'static CStr,
}

/// Workbench argument with a file name and a lock to its directory.
#[derive(Default)]
pub struct WbArg {
    /// Lock to the directory of the file.
    ///
    /// This is `None` if the object type does not support locks.
    pub lock: Option<crate::dos::FileLock>,
    /// The name of the file.
    ///
    /// This is `None` if the argument is a directory, disk or Trashcan.
    // note: is it ok to use a static lifetime here, WbStartup should live longer than amidos_main()
    // accessing this name field.
    pub name: Option<&'static core::ffi::CStr>,
}

impl Debug for WbArg {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WbArg")
            .field("lock", &self.lock)
            .field("name", &self.name)
            .finish()
    }
}

/// Iterator for Workbench arguments.
///
/// Part of [`MainArgs`].
pub struct WbArgs {
    dos_lib: *mut amiga_sys::Library,
    wb_arg_ptr: *const amiga_sys::WBArg,
    wb_arg_cnt: usize,
}

impl Iterator for WbArgs {
    type Item = WbArg;

    fn next(&mut self) -> Option<Self::Item> {
        // check if done
        if self.wb_arg_cnt == 0 {
            return None;
        }
        // return arguments from wb_arg_ptr by converting them to FileLocks and CStrs
        unsafe {
            let lock_bptr = (*self.wb_arg_ptr).wa_Lock;
            let lock = if lock_bptr != 0 {
                let fl = crate::dos::FileLock::from_raw_bptr(self.dos_lib, lock_bptr, false);
                Some(fl)
            } else {
                None
            };
            let name_ptr = (*self.wb_arg_ptr).wa_Name as *const i8;
            let name = if !name_ptr.is_null() {
                Some(core::ffi::CStr::from_ptr(name_ptr))
            } else {
                None
            };
            self.wb_arg_cnt -= 1;
            // if count is zero, then don't increment the pointer because then it would
            // point outside the WBStartup object (that would be undefined behavior in Rust)
            if self.wb_arg_cnt > 0 {
                self.wb_arg_ptr = self.wb_arg_ptr.wrapping_add(1);
            }
            Some(WbArg { lock, name })
        }
    }
}

impl Debug for WbArgs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WbArgs")
            .field("count", &self.wb_arg_cnt)
            .finish()
    }
}

/// Library version information.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Default)]
pub struct Version {
    /// Version.
    pub version: u16,
    /// Revision.
    pub revision: u16,
}

/// Signal bits for break keys.
///
/// These are used by the [`poll_break_signals()`] function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BreakSignals(pub u32);

impl BreakSignals {
    /// Signal bit for Ctrl+C.
    pub const CTRL_C: u32        = amiga_sys::SIGBREAKF_CTRL_C;
    /// Signal bit for Ctrl+D.
    pub const CTRL_D: u32        = amiga_sys::SIGBREAKF_CTRL_D;
    /// Signal bit for Ctrl+E.
    pub const CTRL_E: u32        = amiga_sys::SIGBREAKF_CTRL_E;
    /// Signal bit for Ctrl+F.
    pub const CTRL_F: u32        = amiga_sys::SIGBREAKF_CTRL_F;
}

/// Polls for the Ctrl+C, Ctrl+D, Ctrl+E and Ctrl+F break signals.
///
/// Checks if any of `signals` have been received after the last check.
/// Returns `None` if no signals were received. If signals have been received, returns `Some`
/// with the received signals. This method returns immediately with the result and does not block.
///
/// Typically, these signals are received when the user presses Ctrl+C, Ctrl+D, Ctrl+E or Ctrl+F.
///
/// # Examples
///
/// Checks if the Ctrl+C key was pressed.
///
/// ```no_run
/// use amidos::{poll_break_signals, BreakSignals};
/// if poll_break_signals(BreakSignals(BreakSignals::CTRL_C | BreakSignals::CTRL_D)).is_some() {
///     // print "***BREAK" and exit the program by returning from main
/// }
/// ```
pub fn poll_break_signals(signals: BreakSignals) -> Option<BreakSignals> {
    // check and clear CTRL_C/D/E/F signals according to the `signals` value
    let result = unsafe { amiga_sys::SetSignal(amiga_sys::abs_exec_library(), 0, signals.0) };
    if result != 0 {
        Some(BreakSignals(result))
    } else {
        None
    }
}

/// Returns the total stack size in bytes.
pub fn stack_size() -> usize {
    unsafe {
        _INTERNAL_STACK_SIZE
    }
}

/// Returns the remaining free stack size in bytes.
///
/// If the value is a negative value, then a stack overflow may have happened.
/// If the value is greater than [`stack_size()`], then a stack underflow has happened.
///
/// The exec context switching stores registers and other data to the stack, so ensure
/// that the stack has always at least 160 bytes free for that.
pub fn stack_remaining() -> isize {
    unsafe {
        current_stack_pointer()
            .wrapping_add(_INTERNAL_STACK_SIZE)
            .wrapping_sub(_INTERNAL_UPPER_STACK_POINTER)
            .cast_signed()
    }
}

fn current_stack_pointer() -> usize {
    let sptr: usize;
    unsafe {
        asm!(
            // output stack pointer
            "move.l %sp, %d0",
            out("d0") sptr,
        );
    }
    sptr
}

#[allow(clippy::cast_possible_truncation)]
fn i32_from_usize(value: usize) -> i32 {
    value as i32
}

#[allow(clippy::cast_possible_truncation)]
fn u32_from_usize(value: usize) -> u32 {
    value as u32
}

#[allow(clippy::cast_sign_loss)]
fn usize_from_i32(value: i32) -> usize {
    value as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = to_hex(2);
        assert_eq!(result, 50);
    }
}
