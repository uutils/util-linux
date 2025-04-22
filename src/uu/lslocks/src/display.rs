// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::sync::atomic::AtomicPtr;
use std::{io, ptr};

use crate::errors::LsLocksError;
use crate::utils::LockInfo;

fn decimal_point() -> &'static str {
    use std::sync::atomic::Ordering;

    static DEFAULT: &CStr = c".";
    static VALUE: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

    let mut decimal_point = VALUE.load(Ordering::Acquire);

    if decimal_point.is_null() {
        decimal_point = unsafe { libc::localeconv().as_ref() }
            .and_then(|lc| (!lc.decimal_point.is_null()).then_some(lc.decimal_point))
            .unwrap_or(DEFAULT.as_ptr().cast_mut())
            .cast();

        match VALUE.compare_exchange(
            ptr::null_mut(),
            decimal_point,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_previous_value) => {}
            Err(previous_value) => decimal_point = previous_value,
        }
    }

    unsafe { CStr::from_ptr(decimal_point.cast()) }
        .to_str()
        .unwrap()
}

// returns exponent (2^x=n) in range KiB..EiB (2^10..2^60).
fn bytes_exponent(bytes: u64) -> u64 {
    for shift in (10..=60).step_by(10) {
        if bytes < (1 << shift) {
            return shift - 10;
        }
    }
    60
}

fn size_to_human_string(bytes: u64) -> String {
    static LETTERS: [char; 7] = ['B', 'K', 'M', 'G', 'T', 'P', 'E'];

    let exp = bytes_exponent(bytes);
    let unit = LETTERS[if exp == 0 { 0 } else { (exp / 10) as usize }];
    let mut decimal = if exp == 0 { bytes } else { bytes / (1 << exp) };
    let mut fractional = if exp == 0 { 0 } else { bytes % (1 << exp) };

    if fractional != 0 {
        fractional = if fractional >= (u64::MAX / 1000) {
            ((fractional / 1024) * 1000) / (1 << (exp - 10))
        } else {
            (fractional * 1000) / (1 << exp)
        };

        fractional = ((fractional + 50) / 100) * 10;

        if fractional == 100 {
            decimal += 1;
            fractional = 0;
        }
    }

    if fractional == 0 {
        format!("{decimal}{unit}")
    } else {
        format!("{decimal}{}{fractional:02}{unit}", decimal_point())
    }
}

pub(crate) fn describe_integer<T: ToString>(n: T) -> Option<Cow<'static, CStr>> {
    Some(Cow::Owned(CString::new(n.to_string()).unwrap()))
}

pub(crate) fn describe_size(size: u64, in_bytes: bool) -> Option<Cow<'static, CStr>> {
    let value = if in_bytes {
        size.to_string()
    } else {
        size_to_human_string(size)
    };

    Some(Cow::Owned(CString::new(value).unwrap()))
}

pub(crate) fn describe_holders(
    proc_lock: &LockInfo,
    pid_locks: &[LockInfo],
) -> Result<CString, LsLocksError> {
    let lock_compare = move |lock: &&LockInfo| {
        lock.range == proc_lock.range
            && lock.inode == proc_lock.inode
            && lock.device_id == proc_lock.device_id
            && lock.mandatory == proc_lock.mandatory
            && lock.blocked == proc_lock.blocked
            && lock.kind == proc_lock.kind
            && lock.mode == proc_lock.mode
    };

    let mut separator: &[u8] = &[];

    let append_holder = move |mut buffer: Vec<u8>, lock: &LockInfo| {
        buffer.extend(separator);
        separator = b"\n";

        buffer.extend(lock.process_id.to_string().into_bytes());
        buffer.push(b',');

        if let Some(command_line) = lock.command_name.as_deref().map(CStr::to_bytes) {
            buffer.extend(command_line);
        }

        buffer.push(b',');
        buffer.extend(lock.file_descriptor.to_string().into_bytes());
        buffer
    };

    let buffer = pid_locks
        .iter()
        .filter(lock_compare)
        .fold(Vec::default(), append_holder);

    CString::new(buffer).map_err(|_| LsLocksError::io0("invalid data", io::ErrorKind::InvalidData))
}
