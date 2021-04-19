use std::ops::{Deref, DerefMut};

use backtrace::{Backtrace, BacktraceFrame};

const FRAME_OFFSET: usize = 3;

#[derive(Debug)]
pub struct RwLock<T>(tokio::sync::RwLock<T>);

impl<'a, T> RwLock<T> {
    pub fn new(inner: T) -> Self {
        Self(tokio::sync::RwLock::new(inner))
    }

    pub async fn read(&'a self) -> RwLockReadGuard<'a, T> {
        RwLockReadGuard::new(self.0.read().await)
    }

    pub async fn write(&'a self) -> RwLockWriteGuard<'a, T> {
        RwLockWriteGuard::new(self.0.write().await)
    }
}

#[derive(Debug)]
pub struct RwLockReadGuard<'a, T>(tokio::sync::RwLockReadGuard<'a, T>);

impl<'a, T> RwLockReadGuard<'a, T> {
    pub fn new(inner: tokio::sync::RwLockReadGuard<'a, T>) -> Self {
        log_backtrace("[READ] Acquire");
        Self(inner)
    }
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        log_backtrace("[READ] Release");
    }
}

impl<'a, T> Deref for RwLockReadGuard<'a, T> {
    type Target = tokio::sync::RwLockReadGuard<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> DerefMut for RwLockReadGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut tokio::sync::RwLockReadGuard<'a, T> {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct RwLockWriteGuard<'a, T>(tokio::sync::RwLockWriteGuard<'a, T>);

impl<'a, T> RwLockWriteGuard<'a, T> {
    pub fn new(inner: tokio::sync::RwLockWriteGuard<'a, T>) -> Self {
        log_backtrace("[WRITE] Acquire");
        Self(inner)
    }
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        log_backtrace("[WRITE] Release");
    }
}

impl<'a, T> Deref for RwLockWriteGuard<'a, T> {
    type Target = tokio::sync::RwLockWriteGuard<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut tokio::sync::RwLockWriteGuard<'a, T> {
        &mut self.0
    }
}

fn log_backtrace(message: &str) {
    let this_file = file!();
    let mut trace = Backtrace::new();
    trace.resolve();

    let symbols = trace
        .frames()
        .iter()
        .flat_map(BacktraceFrame::symbols)
        .skip_while(|s| {
            s.filename()
             .map(|p| !p.ends_with(this_file))
             .unwrap_or(true)
        })
        .enumerate()
        .filter(|&(i, s)| {
            i >= FRAME_OFFSET &&
            s.filename().map_or(false, |p|
                !p.to_string_lossy().contains(".cargo") &&
                !p.to_string_lossy().contains(".rustup") &&
                !p.to_string_lossy().contains("/rustc")
            )
        });

    let output = symbols.fold(String::new(), |acc, (i, s)| {
        acc + format!(
            "[{}] {}:{}\n",
            i - FRAME_OFFSET,
            s.filename().unwrap().to_string_lossy(),
            s.lineno().unwrap(),
        )
        .as_str()
    });

    log::warn!("{}:\n{}", message, output);
}
