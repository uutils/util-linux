// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, c_int, c_uint, c_void};
use std::ptr::NonNull;
use std::{io, mem, ptr};

use smartcols_sys::{
    SCOLS_ITER_BACKWARD, SCOLS_ITER_FORWARD, libscols_cell, libscols_column, libscols_iter,
    libscols_line, libscols_table, scols_cell_get_data, scols_free_iter, scols_init_debug,
    scols_line_get_cell, scols_line_get_userdata, scols_line_set_data, scols_line_set_userdata,
    scols_new_iter, scols_new_table, scols_print_table, scols_table_enable_export,
    scols_table_enable_json, scols_table_enable_noheadings, scols_table_enable_raw,
    scols_table_enable_shellvar, scols_table_get_line, scols_table_new_column,
    scols_table_new_line, scols_table_next_column, scols_table_set_column_separator,
    scols_table_set_name, scols_unref_table,
};

use crate::errors::LsIpcError;

pub(crate) fn initialize() {
    unsafe { scols_init_debug(0) };
}

#[repr(transparent)]
pub(crate) struct TableRef(NonNull<libscols_table>);

impl From<NonNull<libscols_table>> for TableRef {
    fn from(value: NonNull<libscols_table>) -> Self {
        Self(value)
    }
}

impl TableOperations for TableRef {
    fn as_ptr(&self) -> *mut libscols_table {
        self.0.as_ptr()
    }
}

#[repr(transparent)]
pub(crate) struct Table(NonNull<libscols_table>);

impl Table {
    pub(crate) fn new() -> Result<Self, LsIpcError> {
        NonNull::new(unsafe { scols_new_table() })
            .ok_or_else(|| LsIpcError::io0("scols_new_table", io::ErrorKind::OutOfMemory))
            .map(Self)
    }

    pub(crate) fn into_inner(self) -> NonNull<libscols_table> {
        let ptr = self.0;
        mem::forget(self);
        ptr
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

    fn enable_headings(&mut self, enable: bool) -> Result<(), LsIpcError> {
        let no_headings = c_int::from(!enable);
        let r = unsafe { scols_table_enable_noheadings(self.as_ptr(), no_headings) };
        LsIpcError::io_from_neg_errno("scols_table_enable_noheadings", r).map(|_| ())
    }

    fn enable_shell_variable(&mut self, enable: bool) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_enable_shellvar(self.as_ptr(), c_int::from(enable)) };
        LsIpcError::io_from_neg_errno("scols_table_enable_shellvar", r).map(|_| ())
    }

    fn enable_export(&mut self, enable: bool) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_enable_export(self.as_ptr(), c_int::from(enable)) };
        LsIpcError::io_from_neg_errno("scols_table_enable_export", r).map(|_| ())
    }

    fn enable_raw(&mut self, enable: bool) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_enable_raw(self.as_ptr(), c_int::from(enable)) };
        LsIpcError::io_from_neg_errno("scols_table_enable_raw", r).map(|_| ())
    }

    fn enable_json(&mut self, enable: bool) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_enable_json(self.as_ptr(), c_int::from(enable)) };
        LsIpcError::io_from_neg_errno("scols_table_enable_json", r).map(|_| ())
    }

    fn set_column_separator(&mut self, separator: &CStr) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_set_column_separator(self.as_ptr(), separator.as_ptr()) };
        LsIpcError::io_from_neg_errno("scols_table_set_column_separator", r).map(|_| ())
    }

    fn new_column(
        &mut self,
        name: &CStr,
        width_hint: f64,
        flags: c_uint,
    ) -> Result<ColumnRef, LsIpcError> {
        NonNull::new(unsafe {
            scols_table_new_column(self.as_ptr(), name.as_ptr(), width_hint, flags as c_int)
        })
        .ok_or_else(|| LsIpcError::io0("scols_table_new_column", io::ErrorKind::OutOfMemory))
        .map(ColumnRef)
    }

    fn new_line(&mut self, parent: Option<&mut LineRef>) -> Result<LineRef, LsIpcError> {
        let parent = parent.map_or(ptr::null_mut(), |parent| parent.0.as_ptr());

        NonNull::new(unsafe { scols_table_new_line(self.as_ptr(), parent) })
            .ok_or_else(|| LsIpcError::io0("scols_table_new_line", io::ErrorKind::OutOfMemory))
            .map(LineRef)
    }

    fn set_name(&mut self, name: &CStr) -> Result<(), LsIpcError> {
        let r = unsafe { scols_table_set_name(self.as_ptr(), name.as_ptr()) };
        LsIpcError::io_from_neg_errno("scols_table_set_name", r).map(|_| ())
    }

    fn line(&self, column_index: usize) -> Result<LineRef, LsIpcError> {
        NonNull::new(unsafe { scols_table_get_line(self.as_ptr(), column_index) })
            .ok_or_else(|| LsIpcError::io0("scols_table_get_line", io::ErrorKind::InvalidInput))
            .map(LineRef)
    }

    fn column_iter(&self, direction: IterDirection) -> Result<ColumnIter<'_, Self>, LsIpcError> {
        let iter = NonNull::new(unsafe { scols_new_iter(direction as c_int) })
            .ok_or_else(|| LsIpcError::io0("scols_new_iter", io::ErrorKind::OutOfMemory))?;
        Ok(ColumnIter { table: self, iter })
    }

    fn print(&self) -> Result<(), LsIpcError> {
        let r = unsafe { scols_print_table(self.as_ptr()) };
        LsIpcError::io_from_neg_errno("scols_print_table", r).map(|_| ())
    }
}

#[repr(transparent)]
pub(crate) struct LineRef(NonNull<libscols_line>);

impl LineRef {
    pub(crate) fn user_data(&self) -> *mut c_void {
        unsafe { scols_line_get_userdata(self.0.as_ptr()) }
    }

    pub(crate) fn set_user_data(&mut self, user_data: *mut c_void) -> Result<(), LsIpcError> {
        let r = unsafe { scols_line_set_userdata(self.0.as_ptr(), user_data) };
        LsIpcError::io_from_neg_errno("scols_line_set_userdata", r).map(|_| ())
    }

    pub(crate) fn set_data(&mut self, cell_index: usize, data: &CStr) -> Result<(), LsIpcError> {
        let r = unsafe { scols_line_set_data(self.0.as_ptr(), cell_index, data.as_ptr()) };
        LsIpcError::io_from_neg_errno("scols_line_set_data", r).map(|_| ())
    }

    pub(crate) fn cell(&self, cell_index: usize) -> Result<CellRef, LsIpcError> {
        NonNull::new(unsafe { scols_line_get_cell(self.0.as_ptr(), cell_index) })
            .ok_or_else(|| LsIpcError::io0("scols_line_get_cell", io::ErrorKind::InvalidInput))
            .map(CellRef)
    }
}

#[repr(transparent)]
pub(crate) struct ColumnRef(NonNull<libscols_column>);

#[repr(transparent)]
pub(crate) struct CellRef(NonNull<libscols_cell>);

impl CellRef {
    pub(crate) fn data_as_c_str(&self) -> Option<&CStr> {
        unsafe {
            let data_ptr = scols_cell_get_data(self.0.as_ptr());
            (!data_ptr.is_null()).then(|| CStr::from_ptr(data_ptr))
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub(crate) enum IterDirection {
    Forward = SCOLS_ITER_FORWARD as i32,
    Backward = SCOLS_ITER_BACKWARD as i32,
}

pub(crate) struct ColumnIter<'table, T: TableOperations> {
    table: &'table T,
    iter: NonNull<libscols_iter>,
}

impl<T: TableOperations> Iterator for ColumnIter<'_, T> {
    type Item = Result<ColumnRef, LsIpcError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut column = ptr::null_mut();
        let table_ptr = self.table.as_ptr();
        let r = unsafe { scols_table_next_column(table_ptr, self.iter.as_ptr(), &mut column) };

        match LsIpcError::io_from_neg_errno("scols_table_next_column", r) {
            Err(err) => Some(Err(err)),

            Ok(r) => NonNull::new(column)
                .filter(|_| r == 0)
                .map(ColumnRef)
                .map(Ok),
        }
    }
}

impl<T: TableOperations> Drop for ColumnIter<'_, T> {
    fn drop(&mut self) {
        unsafe { scols_free_iter(self.iter.as_ptr()) }
    }
}
