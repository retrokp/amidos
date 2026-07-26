//! Cave generator cellular automata example CLI program for amidos (Amiga m68k).
//!
//! This example shows how to use the global allocator and heap memory.
//!

#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

amidos::startup_code!();
amidos::panic_handler_abort!();

// prepare a global allocator to use heap memory
extern crate alloc;
amidos::global_allocmem!();
use alloc::{vec, vec::{Vec}};

const WIDTH: usize = 60;
const HEIGHT: usize = 18;
const WALL_CHANCE: u32 = u32::MAX / 2;
const ITERATIONS: usize = 0; // TODO: set this to 3 when rustc/llvm creates correct code for it

// a tiny xorshift32 pseudo-RNG
struct Rng {
    state: u32
}

impl Rng {
    fn new(seed: u32) -> Self {
        Self {
            // xorshift32 cannot have a zero seed
            state: if seed == 0 { 1 } else { seed }
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[inline]
fn idx(x: usize, y: usize) -> usize {
    y * WIDTH + x
}

fn generate_map(map: &mut Vec<bool>, seed: u32) {
    let mut rng = Rng::new(seed);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            map[idx(x, y)] = x == 0 || y == 0 || x == WIDTH - 1 || y == HEIGHT - 1
                || rng.next_u32() < WALL_CHANCE;
        }
    }
}

fn count_wall_neighbors(map: &[bool], x: usize, y: usize) -> usize {
    let mut count = 0;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx >= WIDTH as isize || ny >= HEIGHT as isize {
                count += 1;
            } else if map[idx(nx as usize, ny as usize)] {
                count += 1;
            }
        }
    }
    count
}

fn step(current: &[bool], next: &mut [bool]) {
    next.copy_from_slice(current);
    // preserve borders
    for y in 1..HEIGHT - 1 {
        for x in 1..WIDTH - 1 {
            let walls = count_wall_neighbors(current, x, y);
            next[idx(x, y)] = walls >= 5;
        }
    }
}

fn print_map(output: &mut amidos::File, map: &[bool]) -> Result<(), amidos::Error> {

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if map[idx(x, y)] {
                output.write_all(c"#".to_bytes())?;
            } else {
                output.write_all(c".".to_bytes())?;
            }
        }
        output.write_all(c"\n".to_bytes())?;
    }
    Ok(())
}

fn amidos_main(dos: &mut amidos::Dos, _args: amidos::MainArgs) -> i32 {
    let Some(mut output) = dos.output() else {
        // no output stream: launched from Workbench
        return amidos::EXIT_CODE_ERROR;
    };
    // allocate memory using try_reserve() to catch out-of-memory situations
    let mut current = vec![];
    if current.try_reserve(WIDTH * HEIGHT).is_err() {
        return amidos::EXIT_CODE_ERROR;
    }
    current.resize(WIDTH * HEIGHT, false);

    let mut next = vec![];
    if next.try_reserve(WIDTH * HEIGHT).is_err() {
        return amidos::EXIT_CODE_ERROR;
    }
    next.resize(WIDTH * HEIGHT, false);

    let datestamp = dos.date_stamp();
    let seed = (datestamp.days ^ datestamp.minutes << 6 ^ datestamp.ticks).cast_unsigned();
    generate_map(&mut current, seed);

    for _ in 0..ITERATIONS {
        step(&current, &mut next);
        core::mem::swap(&mut current, &mut next);
    }

    if print_map(&mut output, &current).is_err() {
        return amidos::EXIT_CODE_ERROR;
    }

    amidos::EXIT_CODE_OK
}
