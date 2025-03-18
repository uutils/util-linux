// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::ptr::NonNull;
use std::{io, ptr};

use smartcols_sys::{
    libscols_column, libscols_line, libscols_table, scols_column_set_json_type,
    scols_column_set_safechars, scols_column_set_wrapfunc, scols_init_debug, scols_line_set_data,
    scols_new_table, scols_print_table, scols_table_enable_json, scols_table_enable_noheadings,
    scols_table_enable_raw, scols_table_new_column, scols_table_new_line, scols_table_set_name,
    scols_unref_table,
};

use crate::errors::LsLocksError;

pub(crate) fn initialize() {
    unsafe { scols_init_debug(0) };
}

#[repr(transparent)]
pub(crate) struct Table(NonNull<libscols_table>);

impl Table {
    pub(crate) fn new() -> Result<Self, LsLocksError> {
        NonNull::new(unsafe { scols_new_table() })
            .ok_or_else(|| LsLocksError::io0("scols_new_table", io::ErrorKind::OutOfMemory))
            .map(Self)
    }
}

impl TableOperations for Table {
    fn as_ptr(&self) -> *mut libscols_table {
        self.0.as_ptr()
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        unsafe { scols_unref_table(self.0.as_ptr()) }
    }
}

pub(crate) trait TableOperations: Sized {
    fn as_ptr(&self) -> *mut libscols_table;

    fn enable_headings(&mut self, enable: bool) -> Result<(), LsLocksError> {
        let no_headings = c_int::from(!enable);
        let r = unsafe { scols_table_enable_noheadings(self.as_ptr(), no_headings) };
        LsLocksError::io_from_neg_errno("scols_table_enable_noheadings", r).map(|_| ())
    }

    fn enable_raw(&mut self, enable: bool) -> Result<(), LsLocksError> {
        let r = unsafe { scols_table_enable_raw(self.as_ptr(), c_int::from(enable)) };
        LsLocksError::io_from_neg_errno("scols_table_enable_raw", r).map(|_| ())
    }

    fn enable_json(&mut self, enable: bool) -> Result<(), LsLocksError> {
        let r = unsafe { scols_table_enable_json(self.as_ptr(), c_int::from(enable)) };
        LsLocksError::io_from_neg_errno("scols_table_enable_json", r).map(|_| ())
    }

    fn new_column(
        &mut self,
        name: &CStr,
        width_hint: f64,
        flags: c_uint,
    ) -> Result<ColumnRef, LsLocksError> {
        NonNull::new(unsafe {
            scols_table_new_column(self.as_ptr(), name.as_ptr(), width_hint, flags as c_int)
        })
        .ok_or_else(|| LsLocksError::io0("scols_table_new_column", io::ErrorKind::OutOfMemory))
        .map(ColumnRef)
    }

    fn new_line(&mut self, parent: Option<&mut LineRef>) -> Result<LineRef, LsLocksError> {
        let parent = parent.map_or(ptr::null_mut(), |parent| parent.0.as_ptr());

        NonNull::new(unsafe { scols_table_new_line(self.as_ptr(), parent) })
            .ok_or_else(|| LsLocksError::io0("scols_table_new_line", io::ErrorKind::OutOfMemory))
            .map(LineRef)
    }

    fn set_name(&mut self, name: &CStr) -> Result<(), LsLocksError> {
        let r = unsafe { scols_table_set_name(self.as_ptr(), name.as_ptr()) };
        LsLocksError::io_from_neg_errno("scols_table_set_name", r).map(|_| ())
    }

    fn print(&self) -> Result<(), LsLocksError> {
        let r = unsafe { scols_print_table(self.as_ptr()) };
        LsLocksError::io_from_neg_errno("scols_print_table", r).map(|_| ())
    }
}

#[repr(transparent)]
pub(crate) struct LineRef(NonNull<libscols_line>);

impl LineRef {
    pub(crate) fn set_data(&mut self, cell_index: usize, data: &CStr) -> Result<(), LsLocksError> {
        let r = unsafe { scols_line_set_data(self.0.as_ptr(), cell_index, data.as_ptr()) };
        LsLocksError::io_from_neg_errno("scols_line_set_data", r).map(|_| ())
    }
}

#[repr(transparent)]
pub(crate) struct ColumnRef(NonNull<libscols_column>);

impl ColumnRef {
    pub(crate) fn set_json_type(&mut self, json_type: c_uint) -> Result<(), LsLocksError> {
        let r = unsafe { scols_column_set_json_type(self.0.as_ptr(), json_type as c_int) };
        LsLocksError::io_from_neg_errno("scols_column_set_json_type", r).map(|_| ())
    }

    pub(crate) fn set_safe_chars(&mut self, safe: &CStr) -> Result<(), LsLocksError> {
        let r = unsafe { scols_column_set_safechars(self.0.as_ptr(), safe.as_ptr()) };
        LsLocksError::io_from_neg_errno("scols_column_set_safechars", r).map(|_| ())
    }

    pub(crate) fn set_wrap_func(
        &mut self,
        wrap_chunk_size: Option<
            unsafe extern "C" fn(*const libscols_column, *const c_char, *mut c_void) -> usize,
        >,
        wrap_next_chunk: Option<
            unsafe extern "C" fn(*const libscols_column, *mut c_char, *mut c_void) -> *mut c_char,
        >,
        user_data: *mut c_void,
    ) -> Result<(), LsLocksError> {
        let r = unsafe {
            scols_column_set_wrapfunc(self.0.as_ptr(), wrap_chunk_size, wrap_next_chunk, user_data)
        };
        LsLocksError::io_from_neg_errno("scols_column_set_wrapfunc", r).map(|_| ())
    }
}
