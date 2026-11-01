//! Read-only ramdisk: an archive of files embedded in the kernel image.
//!
//! Format (little endian): "RDSK", u32 count, count * { name[24] NUL padded,
//! u32 offset, u32 size }, then file data. Built by build_user.ps1.

static IMAGE: &[u8] = include_bytes!("../ramdisk.img");
const ENTRY: usize = 32;

fn u32_at(off: usize) -> Option<usize> {
    let b = IMAGE.get(off..off + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

fn count() -> usize {
    if IMAGE.get(0..4) != Some(b"RDSK") {
        return 0;
    }
    u32_at(4).unwrap_or(0)
}

/// Entry `i` as (name, data).
fn entry(i: usize) -> Option<(&'static str, &'static [u8])> {
    let base = 8 + i * ENTRY;
    let raw = IMAGE.get(base..base + 24)?;
    let len = raw.iter().position(|&b| b == 0).unwrap_or(24);
    let name = core::str::from_utf8(&raw[..len]).ok()?;
    let off = u32_at(base + 24)?;
    let size = u32_at(base + 28)?;
    Some((name, IMAGE.get(off..off.checked_add(size)?)?))
}

pub fn find(name: &str) -> Option<&'static [u8]> {
    (0..count()).filter_map(entry).find(|e| e.0 == name).map(|e| e.1)
}

pub fn list() -> impl Iterator<Item = (&'static str, usize)> {
    (0..count()).filter_map(entry).map(|e| (e.0, e.1.len()))
}
