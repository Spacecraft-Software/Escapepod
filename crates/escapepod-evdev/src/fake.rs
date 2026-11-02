// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fake devices for tests of the daemon's logic (feature `fake`).
//!
//! Each fake shares its state with the test through a handle, so the test
//! can queue input, fail a read, unplug a device, or read what was emitted
//! after the daemon has taken ownership.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use escapepod_core::KeyEvent;

use crate::io::{Keyboard, Opener, Pointer, PointerEvent, Sink};

/// The test's view of one fake keyboard.
#[derive(Debug, Default)]
pub struct KeyboardState {
    /// Events the next read returns.
    pub queue: VecDeque<KeyEvent>,
    /// Keys reported down by `keys_down`.
    pub down: usize,
    /// Whether the daemon holds the grab.
    pub grabbed: bool,
    /// An error the next read (and `keys_down`) returns.
    pub fail: Option<io::ErrorKind>,
    /// Whether `grab` fails with `EBUSY`.
    pub busy: bool,
}

/// A shared handle on a fake keyboard's state.
pub type KeyboardHandle = Rc<RefCell<KeyboardState>>;

/// A fake keyboard.
#[derive(Debug)]
pub struct FakeKeyboard(KeyboardHandle);

impl Keyboard for FakeKeyboard {
    fn grab(&mut self) -> io::Result<()> {
        let mut s = self.0.borrow_mut();
        if s.busy {
            return Err(io::Error::from(io::ErrorKind::ResourceBusy));
        }
        s.grabbed = true;
        Ok(())
    }

    fn keys_down(&self) -> io::Result<usize> {
        let s = self.0.borrow();
        match s.fail {
            Some(kind) => Err(io::Error::from(kind)),
            None => Ok(s.down),
        }
    }

    fn read(&mut self, out: &mut Vec<KeyEvent>) -> io::Result<()> {
        let mut s = self.0.borrow_mut();
        if let Some(kind) = s.fail {
            return Err(io::Error::from(kind));
        }
        out.extend(s.queue.drain(..));
        Ok(())
    }
}

impl Drop for FakeKeyboard {
    fn drop(&mut self) {
        // Closing the descriptor drops the grab, as the kernel does.
        self.0.borrow_mut().grabbed = false;
    }
}

/// A fake pointer: the number of presses its next read returns.
#[derive(Debug)]
pub struct FakePointer(Rc<RefCell<usize>>, Rc<RefCell<PointerState>>);

/// A mapped pointer's fake state.
#[derive(Debug, Default)]
pub struct PointerState {
    /// Frames waiting to be read.
    pub events: Vec<PointerEvent>,
    /// Whether its capabilities are supported.
    pub supported: bool,
    /// Whether its buttons are up.
    pub idle: bool,
    /// Whether its grab is held.
    pub grabbed: bool,
    /// A read failure.
    pub fail: Option<io::ErrorKind>,
}

impl Drop for FakePointer {
    fn drop(&mut self) {
        self.1.borrow_mut().grabbed = false;
    }
}

impl Pointer for FakePointer {
    fn supports_forwarding(&self) -> bool {
        self.1.borrow().supported
    }
    fn is_idle(&self) -> io::Result<bool> {
        Ok(self.1.borrow().idle)
    }
    fn grab(&mut self) -> io::Result<()> {
        self.1.borrow_mut().grabbed = true;
        Ok(())
    }
    fn read_events(&mut self, out: &mut Vec<PointerEvent>) -> io::Result<()> {
        let mut state = self.1.borrow_mut();
        if let Some(kind) = state.fail {
            return Err(kind.into());
        }
        out.append(&mut state.events);
        Ok(())
    }
    fn read_presses(&mut self) -> io::Result<usize> {
        Ok(std::mem::take(&mut *self.0.borrow_mut()))
    }
}

/// A fake virtual keyboard recording every batch it is given.
#[derive(Debug, Default, Clone)]
pub struct FakeSink(
    pub Rc<RefCell<Vec<KeyEvent>>>,
    pub Rc<RefCell<Vec<PointerEvent>>>,
);

impl Sink for FakeSink {
    fn has_pointer(&self) -> bool {
        true
    }
    fn emit_pointer(&mut self, events: &[PointerEvent]) -> io::Result<()> {
        self.1.borrow_mut().extend_from_slice(events);
        Ok(())
    }
    fn emit(&mut self, events: &[KeyEvent]) -> io::Result<()> {
        self.0.borrow_mut().extend_from_slice(events);
        Ok(())
    }
}

/// A fake opener over a table of paths; records every path opened.
#[derive(Debug, Default, Clone)]
pub struct FakeOpener {
    /// State of mapped pointers by path.
    pub pointer_states: Rc<RefCell<BTreeMap<PathBuf, Rc<RefCell<PointerState>>>>>,
    /// The keyboards by path.
    pub keyboards: Rc<RefCell<BTreeMap<PathBuf, KeyboardHandle>>>,
    /// The pointers by path.
    pub pointers: Rc<RefCell<BTreeMap<PathBuf, Rc<RefCell<usize>>>>>,
    /// Every path opened, in order.
    pub opened: Rc<RefCell<Vec<PathBuf>>>,
}

impl FakeOpener {
    /// Adds a pointer supporting mapped-button tests.
    #[must_use]
    pub fn add_mapped_pointer(&self, path: &str) -> Rc<RefCell<PointerState>> {
        let _ = self.add_pointer(path);
        let state = Rc::new(RefCell::new(PointerState {
            supported: true,
            idle: true,
            ..PointerState::default()
        }));
        self.pointer_states
            .borrow_mut()
            .insert(path.into(), Rc::clone(&state));
        state
    }
    /// Adds a keyboard at `path` and returns its handle.
    #[must_use]
    pub fn add_keyboard(&self, path: &str) -> KeyboardHandle {
        let h = KeyboardHandle::default();
        self.keyboards
            .borrow_mut()
            .insert(PathBuf::from(path), Rc::clone(&h));
        h
    }

    /// Adds a pointer at `path` and returns its press counter.
    #[must_use]
    pub fn add_pointer(&self, path: &str) -> Rc<RefCell<usize>> {
        let h = Rc::new(RefCell::new(0));
        self.pointers
            .borrow_mut()
            .insert(PathBuf::from(path), Rc::clone(&h));
        h
    }
}

impl Opener for FakeOpener {
    type Keyboard = FakeKeyboard;
    type Pointer = FakePointer;

    fn keyboard(&mut self, path: &Path) -> io::Result<FakeKeyboard> {
        self.opened.borrow_mut().push(path.to_path_buf());
        self.keyboards
            .borrow()
            .get(path)
            .map(|h| FakeKeyboard(Rc::clone(h)))
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn pointer(&mut self, path: &Path) -> io::Result<FakePointer> {
        self.opened.borrow_mut().push(path.to_path_buf());
        self.pointers
            .borrow()
            .get(path)
            .map(|h| {
                FakePointer(
                    Rc::clone(h),
                    self.pointer_states
                        .borrow()
                        .get(path)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

// Rust guideline compliant 2026-05-18
