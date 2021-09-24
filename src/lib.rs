use std::{marker::PhantomData, ops::{Deref, DerefMut}, sync::atomic::{AtomicU64, Ordering}};

use backtrace::{Backtrace, BacktraceFrame};
use names::Generator;

const FRAME_OFFSET: usize = 3;

#[derive(Debug)]
pub struct RwLock<T> {
    lock: tokio::sync::RwLock<T>,
    name: String,
    idx: AtomicU64,
}

impl<'a, T: ?Sized> RwLock<T>
where
    T: Sized,
{
    pub fn new(inner: T) -> RwLock<T> {
        let mut generator = Generator::default();

        Self {
            lock: tokio::sync::RwLock::new(inner),
            name: generator.next().unwrap(),
            idx: AtomicU64::new(0),
        }
    }

    pub async fn read(&'a self) -> RwLockReadGuard<'a, T> {
        self.idx.fetch_add(1, Ordering::SeqCst);
        let idx = self.idx.load(Ordering::SeqCst);
        log_backtrace(&format!("[READ] Acquire ({}:{})", self.name, idx));
        RwLockReadGuard::new(self.lock.read().await, &self.name, idx)
    }

    pub async fn write(&'a self) -> RwLockWriteGuard<'a, T> {
        self.idx.fetch_add(1, Ordering::SeqCst);
        let idx = self.idx.load(Ordering::SeqCst);
        log_backtrace(&format!("[WRITE] Acquire ({}:{})", self.name, idx));
        RwLockWriteGuard::new(self.lock.write().await, &self.name, idx)
    }
}

#[derive(Debug)]
pub struct RwLockReadGuard<'a, T: ?Sized> {
    guard: tokio::sync::RwLockReadGuard<'a, T>,
    name: &'a str,
    idx: u64,
}

impl<'a, T: ?Sized> RwLockReadGuard<'a, T> {
    pub fn new(inner: tokio::sync::RwLockReadGuard<'a, T>, name: &'a str, idx: u64) -> Self {
        let new = Self { guard: inner, name, idx };
        log_backtrace(&format!("[READ] Got ({}:{})", name, idx));
        new
    }
}

impl<'a, T: ?Sized> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        log_backtrace(&format!("[READ] Release ({}:{})", self.name, self.idx));
    }
}

impl<'a, T: ?Sized> Deref for RwLockReadGuard<'a, T> {
    type Target = tokio::sync::RwLockReadGuard<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T: ?Sized> DerefMut for RwLockReadGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut tokio::sync::RwLockReadGuard<'a, T> {
        &mut self.guard
    }
}

#[derive(Debug)]
pub struct RwLockMappedWriteGuard<'a, T: ?Sized> {
    data: *mut T,
    marker: PhantomData<&'a mut T>,
}

impl<T: ?Sized> Deref for RwLockMappedWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized> DerefMut for RwLockMappedWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

#[derive(Debug)]
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    guard: tokio::sync::RwLockWriteGuard<'a, T>,
    name: &'a str,
    idx: u64,
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    pub fn new(inner: tokio::sync::RwLockWriteGuard<'a, T>, name: &'a str, idx: u64) -> Self {
        let new = Self { guard: inner, name, idx };
        log_backtrace(&format!("[WRITE] Got ({}:{})", name, idx));
        new
    }

    pub fn try_map<F, U: ?Sized>(
        mut this: Self,
        f: F,
    ) -> Result<RwLockMappedWriteGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut U,
            None => return Err(this),
        };

        std::mem::forget(this);

        Ok(RwLockMappedWriteGuard {
            data,
            marker: PhantomData,
        })
    }
}

impl<'a, T: ?Sized> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        log_backtrace(&format!("[WRITE] Release ({}:{})", self.name, self.idx));
    }
}

impl<'a, T: ?Sized> Deref for RwLockWriteGuard<'a, T> {
    type Target = tokio::sync::RwLockWriteGuard<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a, T: ?Sized> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut tokio::sync::RwLockWriteGuard<'a, T> {
        &mut self.guard
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
            "[{}] {}:{} {}\n",
            i - FRAME_OFFSET,
            s.filename().unwrap().to_string_lossy(),
            s.lineno().unwrap(),
            s.name().unwrap(),
        )
        .as_str()
    });

    log::warn!("{}:\n{}", message, output);
}

unsafe impl<T> Send for RwLockWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Send for RwLockMappedWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Sync for RwLockWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
unsafe impl<T> Sync for RwLockMappedWriteGuard<'_, T> where T: ?Sized + Send + Sync {}
