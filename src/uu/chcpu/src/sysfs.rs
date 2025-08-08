// This file is part of the uutils hostname package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CString, c_int};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write, stdout};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, str};

use crate::errors::ChCpuError;
use crate::{CpuList, DispatchMode};

pub(crate) const PATH_SYS_CPU: &str = "/sys/devices/system/cpu";

pub(crate) struct SysFSCpu(File);

impl SysFSCpu {
    pub(crate) fn open() -> Result<Self, ChCpuError> {
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(PATH_SYS_CPU)
            .map(Self)
            .map_err(|err| ChCpuError::io1("failed to open", PATH_SYS_CPU, err))
    }

    fn inner_path(name: impl AsRef<Path>) -> PathBuf {
        Path::new(PATH_SYS_CPU).join(name)
    }

    pub(crate) fn ensure_accessible(
        &self,
        name: impl AsRef<Path>,
        access: c_int,
    ) -> Result<(), ChCpuError> {
        use std::io::Error;

        let name = name.as_ref();
        let c_name = c_string_from_path(name)?;

        if unsafe { libc::faccessat(self.0.as_raw_fd(), c_name.as_ptr(), access, 0) } == 0 {
            Ok(())
        } else {
            let path = Self::inner_path(name);
            let err = Error::last_os_error();
            Err(ChCpuError::io1("file/directory is inaccessible", path, err))
        }
    }

    pub(crate) fn open_inner(
        &self,
        name: impl AsRef<Path>,
        flags: c_int,
    ) -> Result<File, ChCpuError> {
        use std::io::Error;

        let name = name.as_ref();
        let c_name = c_string_from_path(name)?;

        unsafe {
            let fd = libc::openat(self.0.as_raw_fd(), c_name.as_ptr(), flags);
            if fd >= 0 {
                return Ok(File::from_raw_fd(fd));
            }
        }

        let path = Self::inner_path(name);
        let err = Error::last_os_error();
        Err(ChCpuError::io1("failed to open", path, err))
    }

    pub(crate) fn read_value<T>(&self, name: impl AsRef<Path>) -> Result<T, ChCpuError>
    where
        T: FromStr,
    {
        let name = name.as_ref();
        let mut line = String::default();

        self.open_inner(name, libc::O_RDONLY | libc::O_CLOEXEC)
            .map(BufReader::new)?
            .read_line(&mut line)
            .map_err(|err| ChCpuError::io1("failed to read file", Self::inner_path(name), err))?;

        line.trim()
            .parse()
            .map_err(|_r| ChCpuError::NotInteger(line.trim().into()))
    }

    pub(crate) fn write_value(
        &self,
        name: impl AsRef<Path>,
        value: impl fmt::Display,
    ) -> Result<(), ChCpuError> {
        let name = name.as_ref();

        self.open_inner(name, libc::O_WRONLY | libc::O_CLOEXEC)?
            .write_all(format!("{value}").as_bytes())
            .map_err(|err| ChCpuError::io1("failed to write file", Self::inner_path(name), err))
    }

    pub(crate) fn enabled_cpu_list(&self) -> Result<CpuList, ChCpuError> {
        let mut buffer = Vec::default();

        self.open_inner("online", libc::O_RDONLY | libc::O_CLOEXEC)?
            .read_to_end(&mut buffer)
            .map_err(|err| {
                ChCpuError::io1("failed to read file", Self::inner_path("online"), err)
            })?;

        CpuList::try_from(buffer.as_slice())
    }

    pub(crate) fn cpu_dir_path(&self, cpu_index: usize) -> Result<PathBuf, ChCpuError> {
        let dir_name = PathBuf::from(format!("cpu{cpu_index}"));

        self.ensure_accessible(&dir_name, libc::F_OK)
            .map(|()| dir_name)
            .map_err(|_r| ChCpuError::InvalidCpuIndex(cpu_index))
    }

    pub(crate) fn enable_cpu(
        &self,
        enabled_cpu_list: Option<&mut CpuList>,
        cpu_index: usize,
        enable: bool,
    ) -> Result<(), ChCpuError> {
        use std::ops::RangeInclusive;

        let dir_name = self.cpu_dir_path(cpu_index)?;

        let online_path = dir_name.join("online");
        self.ensure_accessible(&online_path, libc::F_OK)
            .map_err(|_r| ChCpuError::CpuNotHotPluggable(cpu_index))?;

        let online = self
            .read_value::<i32>(&online_path)
            .map(|value| value != 0)?;

        let new_state = if enable { "enabled" } else { "disabled" };

        if enable == online {
            let mut stdout = stdout().lock();
            return writeln!(&mut stdout, "CPU {cpu_index} is already {new_state}")
                .map_err(|err| ChCpuError::io0("write standard output", err));
        }

        if let Some(enabled_cpu_list) = &enabled_cpu_list {
            let iter = enabled_cpu_list
                .0
                .iter()
                .flat_map(RangeInclusive::to_owned)
                .take(2);

            if !enable && iter.count() <= 1 {
                return Err(ChCpuError::OneCpuIsEnabled);
            }
        }

        let configured = self.read_value::<i32>(dir_name.join("configure"));

        if let Err(err) = self.write_value(&online_path, u8::from(enable)) {
            let operation = if enable { "enable" } else { "disable" };

            let reason = if enable && configured.is_ok_and(|value| value == 0) {
                " (CPU is deconfigured)"
            } else {
                ""
            };

            return Err(err.with_io_message(format!("CPU {cpu_index} {operation} failed{reason}")));
        }

        if let Some(enabled_cpu_list) = enabled_cpu_list {
            if enable {
                enabled_cpu_list.0.insert(cpu_index..=cpu_index);
            } else {
                enabled_cpu_list.0.remove(cpu_index..=cpu_index);
            }
        }

        let mut stdout = stdout().lock();
        writeln!(&mut stdout, "CPU {cpu_index} {new_state}",)
            .map_err(|err| ChCpuError::io0("write standard output", err))
    }

    pub(crate) fn configure_cpu(
        &self,
        enabled_cpu_list: Option<&CpuList>,
        cpu_index: usize,
        configure: bool,
    ) -> Result<(), ChCpuError> {
        let dir_name = self.cpu_dir_path(cpu_index)?;

        let configure_path = dir_name.join("configure");
        self.ensure_accessible(&configure_path, libc::F_OK)
            .map_err(|_r| ChCpuError::CpuNotConfigurable(cpu_index))?;

        let previous_config = self
            .read_value::<i32>(&configure_path)
            .map(|value| value != 0)?;

        let new_state = if configure {
            "configured"
        } else {
            "deconfigured"
        };

        if configure == previous_config {
            let mut stdout = stdout().lock();
            return writeln!(&mut stdout, "CPU {cpu_index} is already {new_state}")
                .map_err(|err| ChCpuError::io0("write standard output", err));
        }

        if let Some(enabled_cpu_list) = enabled_cpu_list
            && previous_config
            && !configure
            && enabled_cpu_list.0.contains(&cpu_index)
        {
            return Err(ChCpuError::CpuIsEnabled(cpu_index));
        }

        if let Err(err) = self.write_value(&configure_path, u8::from(configure)) {
            let operation = if configure {
                "configure"
            } else {
                "deconfigure"
            };
            Err(err.with_io_message(format!("CPU {cpu_index} {operation} failed")))
        } else {
            let mut stdout = stdout().lock();
            writeln!(&mut stdout, "CPU {cpu_index} {new_state}",)
                .map_err(|err| ChCpuError::io0("write standard output", err))
        }
    }

    pub(crate) fn set_dispatch_mode(&self, mode: DispatchMode) -> Result<(), ChCpuError> {
        self.ensure_accessible("dispatching", libc::F_OK)
            .map_err(|_r| ChCpuError::SetCpuDispatchUnsupported)?;

        self.write_value("dispatching", mode as u8)
            .map_err(|err| err.with_io_message("failed to set dispatch mode"))?;

        let mut stdout = stdout().lock();
        writeln!(&mut stdout, "Successfully set {mode} dispatching mode")
            .map_err(|err| ChCpuError::io0("write standard output", err))
    }

    pub(crate) fn rescan_cpus(&self) -> Result<(), ChCpuError> {
        self.ensure_accessible("rescan", libc::F_OK)
            .map_err(|_r| ChCpuError::CpuRescanUnsupported)?;

        self.write_value("rescan", "1")
            .map_err(|err| err.with_io_message("failed to trigger rescan of CPUs"))?;

        let mut stdout = stdout().lock();
        writeln!(&mut stdout, "Triggered rescan of CPUs")
            .map_err(|err| ChCpuError::io0("write standard output", err))
    }
}

fn c_string_from_path(path: &Path) -> Result<CString, ChCpuError> {
    use std::io::{Error, ErrorKind};

    CString::new(path.as_os_str().as_bytes())
        .map_err(|_r| ChCpuError::io1("invalid name", path, Error::from(ErrorKind::InvalidInput)))
}
