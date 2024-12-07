//! This crate is for creating utilities that aren't crate specific.
#![feature(maybe_uninit_array_assume_init)]
#![feature(bufreader_peek)]
pub mod bufread;
pub mod iters;
pub mod other;
