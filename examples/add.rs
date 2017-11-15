#![feature(proc_macro)]

extern crate accel;
extern crate accel_derive;

use accel_derive::kernel;
use accel::*;

#[kernel]
pub unsafe fn add(a: *const f64, b: *const f64, c: *mut f64, n: usize) {
    let i = accel_core::index();
    if (i as usize) < n {
        *c.offset(i) = *a.offset(i) + *b.offset(i);
    }
}

fn main() {
    let n = 1024;
    let a = UVec::new(n).unwrap();
    let b = UVec::new(n).unwrap();
    let mut c = UVec::new(n).unwrap();

    let grid = Grid::x(64);
    let block = Block::x(64);
    add(grid, block, a.as_ptr(), b.as_ptr(), c.as_mut_ptr(), n);
}
