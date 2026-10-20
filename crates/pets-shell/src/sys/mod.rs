//! Платформенный код: единственный экземпляр и журнал событий ОС. `unsafe` — только здесь.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub(crate) use other::*;
