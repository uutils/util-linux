// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, c_int, c_uint};
use std::ptr::NonNull;
use std::{io, ptr};

use smartcols_sys::{
    libscols_column, libscols_line, libscols_table, scols_init_debug, scols_line_set_data,
    scols_new_table, scols_print_table, scols_table_new_column, scols_table_new_line,
    scols_unref_table,
};

use crate::errors::LsnsError;

pub(crate) fn initialize() {
    unsafe { scols_init_debug(0) };
}

#[repr(transparent)]
pub(crate) struct Table(NonNull<libscols_table>);

impl Table {
    pub(crate) fn new() -> Result<Self, LsnsError> {
        NonNull::new(unsafe { scols_new_table() })
            .ok_or_else(|| LsnsError::io0("scols_new_table", io::ErrorKind::OutOfMemory))
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

    fn new_column(
        &mut self,
        name: &CStr,
        width_hint: f64,
        flags: c_uint,
    ) -> Result<ColumnRef, LsnsError> {
        NonNull::new(unsafe {
            scols_table_new_column(self.as_ptr(), name.as_ptr(), width_hint, flags as c_int)
        })
        .ok_or_else(|| LsnsError::io0("scols_table_new_column", io::ErrorKind::OutOfMemory))
        .map(ColumnRef)
    }

    fn new_line(&mut self, parent: Option<&mut LineRef>) -> Result<LineRef, LsnsError> {
        let parent = parent.map_or(ptr::null_mut(), |parent| parent.0.as_ptr());

        NonNull::new(unsafe { scols_table_new_line(self.as_ptr(), parent) })
            .ok_or_else(|| LsnsError::io0("scols_table_new_line", io::ErrorKind::OutOfMemory))
            .map(LineRef)
    }

    fn print(&self) -> Result<(), LsnsError> {
        let r = unsafe { scols_print_table(self.as_ptr()) };
        LsnsError::io_from_neg_errno("scols_print_table", r).map(|_| ())
    }
}

#[repr(transparent)]
pub(crate) struct LineRef(NonNull<libscols_line>);

impl LineRef {
    pub(crate) fn set_data(&mut self, cell_index: usize, data: &CStr) -> Result<(), LsnsError> {
        let r = unsafe { scols_line_set_data(self.0.as_ptr(), cell_index, data.as_ptr()) };
        LsnsError::io_from_neg_errno("scols_line_set_data", r).map(|_| ())
    }
}

#[repr(transparent)]
pub(crate) struct ColumnRef(NonNull<libscols_column>);
