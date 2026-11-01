// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device discovery from sysfs, without opening any event device.
//!
//! Each `/sys/class/input/event<N>` directory names a device node
//! `/dev/input/event<N>`; its `device/` directory carries the name, the
//! vendor and product identifiers and the key capability bitmap. Reading
//! them needs no access to the device itself (ESC-SRS-036).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The name prefix of every virtual keyboard Escape Pod creates. A device
/// whose name starts with it is never opened (ESC-SRS-045).
pub const OWN_NAME_PREFIX: &str = "Escape Pod virtual keyboard";

/// The vendor identifier Escape Pod gives its virtual keyboard: ASCII
/// `SP`. Together with [`OWN_PRODUCT`] it marks a device as Escape Pod's
/// even if its name was changed.
pub const OWN_VENDOR: u16 = 0x5350;

/// The product identifier Escape Pod gives its virtual keyboard: ASCII
/// `EP`.
pub const OWN_PRODUCT: u16 = 0x4550;

/// What kind of input device a node is, judged from its key capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// Reports the letter keys, Enter and Space.
    Keyboard,
    /// Reports a left pointer button.
    Pointer,
    /// Anything else (power buttons, lid switches, sensors).
    Other,
}

impl DeviceKind {
    /// The lowercase name used in output: `keyboard`, `pointer`, `other`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Pointer => "pointer",
            Self::Other => "other",
        }
    }
}

/// One input device as sysfs describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The device node, such as `/dev/input/event3`.
    pub path: PathBuf,
    /// The kernel's name for the device.
    pub name: String,
    /// The USB or bus vendor identifier.
    pub vendor: u16,
    /// The USB or bus product identifier.
    pub product: u16,
    /// Keyboard, pointer or other.
    pub kind: DeviceKind,
}

impl DeviceInfo {
    /// Whether this is a virtual keyboard an Escape Pod process created.
    #[must_use]
    pub fn is_own(&self) -> bool {
        self.name.starts_with(OWN_NAME_PREFIX)
            || (self.vendor == OWN_VENDOR && self.product == OWN_PRODUCT)
    }
}

/// Lists every `event<N>` device under `sysfs_class` (normally
/// `/sys/class/input`), with device nodes under `dev_dir` (normally
/// `/dev/input`), sorted by event number. Directories that cannot be read
/// are skipped: they vanish when a device is unplugged mid-scan.
///
/// # Errors
///
/// Returns the I/O error when `sysfs_class` itself cannot be read.
pub fn scan(sysfs_class: &Path, dev_dir: &Path) -> io::Result<Vec<DeviceInfo>> {
    let mut found: Vec<(u32, DeviceInfo)> = Vec::new();
    for entry in fs::read_dir(sysfs_class)? {
        let Ok(entry) = entry else { continue };
        let file_name = entry.file_name();
        let Some(node) = file_name.to_str() else {
            continue;
        };
        let Some(n) = node
            .strip_prefix("event")
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(info) = read_device(&entry.path().join("device"), dev_dir.join(node)) {
            found.push((n, info));
        }
    }
    found.sort_by_key(|(n, _)| *n);
    Ok(found.into_iter().map(|(_, d)| d).collect())
}

/// Describes the one device node `node` (such as `event3`), as [`scan`]
/// would; `None` when sysfs has no such device.
#[must_use]
pub fn info(sysfs_class: &Path, dev_dir: &Path, node: &str) -> Option<DeviceInfo> {
    node.strip_prefix("event")?.parse::<u32>().ok()?;
    read_device(&sysfs_class.join(node).join("device"), dev_dir.join(node))
}

/// Reads one device's sysfs attributes; `None` when they are missing.
fn read_device(dir: &Path, path: PathBuf) -> Option<DeviceInfo> {
    let name = fs::read_to_string(dir.join("name"))
        .ok()?
        .trim_end()
        .to_owned();
    let vendor = read_hex(&dir.join("id/vendor")).unwrap_or(0);
    let product = read_hex(&dir.join("id/product")).unwrap_or(0);
    let keys = fs::read_to_string(dir.join("capabilities/key")).unwrap_or_default();
    Some(DeviceInfo {
        path,
        name,
        vendor,
        product,
        kind: classify(&keys),
    })
}

fn read_hex(path: &Path) -> Option<u16> {
    u16::from_str_radix(fs::read_to_string(path).ok()?.trim(), 16).ok()
}

/// Key codes a keyboard reports: `KEY_A`, `KEY_Z`, `KEY_ENTER`, `KEY_SPACE`.
const KEYBOARD_KEYS: [u16; 4] = [30, 44, 28, 57];
/// `BTN_LEFT`.
const POINTER_BUTTON: u16 = 0x110;

/// Classifies a device from its sysfs key capability bitmap.
fn classify(bitmap: &str) -> DeviceKind {
    if KEYBOARD_KEYS.iter().all(|&k| has_bit(bitmap, k)) {
        DeviceKind::Keyboard
    } else if has_bit(bitmap, POINTER_BUTTON) {
        DeviceKind::Pointer
    } else {
        DeviceKind::Other
    }
}

/// Tests bit `bit` of a sysfs bitmap: space-separated hexadecimal words,
/// most significant first, each `usize::BITS` wide.
fn has_bit(bitmap: &str, bit: u16) -> bool {
    let width = usize::BITS;
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    let index = u32::from(bit) / width;
    let Some(pos) = words.len().checked_sub(1 + index as usize) else {
        return false;
    };
    usize::from_str_radix(words[pos], 16).is_ok_and(|w| w & (1 << (u32::from(bit) % width)) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keyboard bitmap as the kernel prints it on a 64-bit machine.
    const KEYBOARD: &str = "402000000 3803078f800d001 feffffdfffefffff fffffffffffffffe";
    /// A touchpad: `BTN_LEFT` and touch buttons.
    const TOUCHPAD: &str = "6420 30000 0 0 0 0";

    #[test]
    fn classifies_bitmaps() {
        assert_eq!(classify(KEYBOARD), DeviceKind::Keyboard);
        assert_eq!(classify(TOUCHPAD), DeviceKind::Pointer);
        assert_eq!(classify("0"), DeviceKind::Other);
        assert_eq!(classify(""), DeviceKind::Other);
    }

    #[test]
    fn own_devices_by_name_or_ids() {
        let mut d = DeviceInfo {
            path: PathBuf::from("/dev/input/event9"),
            name: format!("{OWN_NAME_PREFIX} 1234"),
            vendor: 1,
            product: 2,
            kind: DeviceKind::Keyboard,
        };
        assert!(d.is_own());
        d.name = "renamed".into();
        assert!(!d.is_own());
        (d.vendor, d.product) = (OWN_VENDOR, OWN_PRODUCT);
        assert!(d.is_own());
    }
}

// Rust guideline compliant 2026-05-18
