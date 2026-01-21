//! Architecture-specific implementations.
//!
//! This module provides platform abstractions for different target architectures.
//! Currently supported: x86_64.

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
