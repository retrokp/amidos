
# Amidos

Unofficial safe API for AmigaDOS to create CLI programs for the Classic Amiga (m68k).

Tested on rustc 1.99.0-nightly (87e5904f5 2026-07-20) and cargo 1.99.0-nightly
(3efb1f477 2026-07-17).

Note: if you want to build high performance games or demos, have a look at
[amiga-rust](https://github.com/grahambates/amiga-rust).

## WARNING: this is an *experimental* crate built on top of the *experimental* Rust m68k toolchain!

It is likely that programs developed with this will crash or simply refuse to
compile, because many pieces are still missing:

 - the LLVM compiler has only experimental support for m68k: it may produce invalid code or
   crash during compilation because of [many LLVM bugs for m68k](https://github.com/llvm/llvm-project/issues?q=state%3Aopen%20label%3A%22backend%3Am68k%22)
 - there's [many Rust bugs for m68k](https://github.com/rust-lang/rust/issues?q=is%3Aissue%20label%3AO-motorola68k%20state%3Aopen)
 - this crate isn't well tested: it may have bugs and corrupt memory

If you still want to try this out, then read on..

## Features

 - safe API to the AmigaDOS system functions (and few exec functions)
   - access command line arguments
   - read and write stdio
   - read and write files
   - use file locks
   - rename files, remove files, access file properties, ..
   - check for CTRL-C key presses
 - supports Amiga m68k Kickstart 1.0-3.x, prioritizes on Kickstart 1.0
 - the API is still a prototype and will change
 - no dependency to the Amiga Native Development Kit (NDK): no dependency to the NDK headers
   or amiga.lib
 - supports `no_std` and `alloc` (`std` isn't available)
 - only cross-compiling for Amiga (no building on Amiga)
 - extra feature: a lazy developer who doesn't respond fast to issues or pull requests because
   this is just a hobby project

## Not supported

 - no other libraries than AmigaDOS
 - no third-party libraries or devices
 - no Rust standard `std` library features, such as println!(), std::io or std::fs
 - no support for AmigaOS 4.0 or other derivatives, PowerPC or other non-m68k Amiga versions
 - no direct access to hardware

## Prerequisites

Building programs requires:

 - Rust nightly: `rustup install nightly` and `rustup component add rust-src`.
 - the linker for [m68k-unknown-none-elf](https://doc.rust-lang.org/rustc/platform-support/m68k-unknown-none-elf.html#requirements): `m68k-linux-gnu-ld` (qemu-user-static isn't needed)
 - [elf2hunk](https://github.com/BartmanAbyss/elf2hunk)

## Building the examples

Run these (replace hello-args with the example name):

    cd examples/hello-args
    cargo +nightly build --target m68k-unknown-none-elf --release
    elf2hunk target/m68k-unknown-none-elf/release/hello-args target/m68k-unknown-none-elf/release/hello-args.exe -s
    # Amiga executable: target/m68k-unknown-none-elf/release/hello-args.exe

The release build is usually more successful than the debug build.

For minimal builds, see the hello-args example's Cargo.toml and .cargo/config.toml flags.
Also, remove `Debug` data with `RUSTFLAGS="-Zfmt-debug=none"` (this doesn't seem to work).
Generic instructions how to minimize Rust builds: https://github.com/johnthagen/min-sized-rust

Note that Rust binaries may include the
[full build path](https://github.com/rust-lang/rust/issues/40552) and stripping
it reduces the file size and increases privacy.

## Design principles

Design principles and the scope of `amidos`:

 - ensure compatibility with Kickstart 1.0
 - directly map to the underlying system API if possible to make it familiar for Amiga developers
 - avoid unnecessary abstractions to minimize code size
 - try to avoid adding extra convenience functions to reduce maintenance burden
 - modernize function, struct and field naming when it makes sense
 - avoid panicking or crashing the system if invalid parameters are given because panicking
   doesn't free memory or resources
 - no raw pointers in public APIs
 - no unsafe keywords in public APIs
 - prevent invalid parameter and flag combinations
 - avoid wasting memory because heap allocations are slow and CLI programs have only
   4000 bytes of stack
 - avoid copying memory because it is slow

## Related

 - [amiga-rust](https://github.com/grahambates/amiga-rust): direct access to hardware
 - [amiga-debug Visual Studio Code Extension](https://github.com/BartmanAbyss/vscode-amiga-debug/tree/master): C/C++ and build tools for Amiga

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-0BSD">0BSD license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
