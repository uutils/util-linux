// This file is part of the uutils coreutils  package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::builder::ValueParser;
use clap::{crate_version, Command};
use clap::{Arg, ArgAction};
use std::path::PathBuf;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("script.md");
const USAGE: &str = help_usage!("script.md");

#[cfg(target_family = "unix")]
mod unix {
    use nix::pty::{openpty, Winsize};
    use nix::sys::termios;
    use nix::unistd::{close, dup2, execvp, fork, ForkResult, Pid};
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Write};
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::macos::fs::MetadataExt;
    use std::os::unix::io::{AsRawFd, RawFd};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;
    use uucore::error::{UResult, USimpleError};

    static FLUSH_LOGS: AtomicBool = AtomicBool::new(false);

    extern "C" fn handle_sigusr1(_: libc::c_int) {
        FLUSH_LOGS.store(true, Ordering::SeqCst);
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum LogFormat {
        Classic,
        Advanced,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum EchoMode {
        Always,
        Never,
        Auto,
    }

    pub struct ScriptOptions {
        pub typescript_file: PathBuf,
        pub append: bool,
        pub command: Option<String>,
        pub echo_mode: EchoMode,
        pub return_exit_status: bool,
        pub flush: bool,
        pub force: bool,
        pub log_io_file: Option<PathBuf>,
        pub log_in_file: Option<PathBuf>,
        pub log_out_file: Option<PathBuf>,
        pub log_timing_file: Option<PathBuf>,
        pub logging_format: LogFormat,
        pub output_limit: Option<u64>,
        pub quiet: bool,
    }

    struct IoHandlerConfig {
        log_format: LogFormat,
        flush: bool,
        output_limit: Option<u64>,
        start_time: Instant,
    }

    struct LogFiles {
        out_file: File,
        log_in_file: Option<File>,
        log_out_file: Option<File>,
        log_io_file: Option<File>,
        timing_file: Option<File>,
    }

    pub fn parse_size(size_str: &str) -> Result<u64, String> {
        let suffixes: HashMap<&str, u64> = [
            ("K", 1024),
            ("KiB", 1024),
            ("M", 1024 * 1024),
            ("MiB", 1024 * 1024),
            ("G", 1024 * 1024 * 1024),
            ("GiB", 1024 * 1024 * 1024),
            ("KB", 1000),
            ("MB", 1000 * 1000),
            ("GB", 1000 * 1000 * 1000),
        ]
        .iter()
        .cloned()
        .collect();

        for (suffix, multiplier) in &suffixes {
            if size_str.ends_with(suffix) {
                let number_str = &size_str[0..size_str.len() - suffix.len()];
                return number_str
                    .parse::<u64>()
                    .map(|n| n * multiplier)
                    .map_err(|_| format!("Invalid number: {}", number_str));
            }
        }

        // No suffix, parse as bytes
        size_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid number: {}", size_str))
    }

    pub fn open_output_file(path: &Path, append: bool, force: bool) -> Result<File, io::Error> {
        if !force && !append {
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.st_nlink() > 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "refusing to output to a file with multiple links",
                    ));
                }
            }
        }

        OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path)
    }

    pub fn run_script(options: ScriptOptions) -> UResult<()> {
        // Set up signal handler for SIGUSR1
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handle_sigusr1 as usize;
            libc::sigemptyset(&mut sa.sa_mask);
            sa.sa_flags = 0;
            if libc::sigaction(libc::SIGUSR1, &sa, std::ptr::null_mut()) < 0 {
                return Err(USimpleError::new(
                    1,
                    "Failed to set up signal handler for SIGUSR1",
                ));
            }
        }

        // Open output files
        let out_file =
            match open_output_file(&options.typescript_file, options.append, options.force) {
                Ok(file) => file,
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to open output file: {}", e),
                    ));
                }
            };

        let log_in_file = if let Some(path) = &options.log_in_file {
            match open_output_file(path, options.append, true) {
                Ok(file) => Some(file),
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to open input log file: {}", e),
                    ));
                }
            }
        } else {
            None
        };

        let log_out_file = if let Some(path) = &options.log_out_file {
            match open_output_file(path, options.append, true) {
                Ok(file) => Some(file),
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to open output log file: {}", e),
                    ));
                }
            }
        } else {
            None
        };

        let log_io_file = if let Some(path) = &options.log_io_file {
            match open_output_file(path, options.append, true) {
                Ok(file) => Some(file),
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to open I/O log file: {}", e),
                    ));
                }
            }
        } else {
            None
        };

        let timing_file = if let Some(path) = &options.log_timing_file {
            match open_output_file(path, options.append, true) {
                Ok(file) => Some(file),
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to open timing log file: {}", e),
                    ));
                }
            }
        } else {
            None
        };

        // Get current terminal settings
        let isatty = unsafe { libc::isatty(libc::STDIN_FILENO) } != 0;

        let termios = if isatty {
            // Create a File from the raw fd (don't actually close the fd when File is dropped)
            let stdin_file = unsafe { File::from_raw_fd(libc::STDIN_FILENO) };

            match termios::tcgetattr(&stdin_file) {
                Ok(t) => {
                    // Prevent the File from closing the fd when it's dropped
                    let _ = stdin_file.into_raw_fd();
                    Some(t)
                }
                Err(e) => {
                    // Prevent the File from closing the fd when it's dropped
                    let _ = stdin_file.into_raw_fd();
                    return Err(USimpleError::new(
                        1,
                        format!("Failed to get terminal attributes: {}", e),
                    ));
                }
            }
        } else {
            None
        };

        // Create a pseudoterminal
        let pty = match openpty(None, None) {
            Ok(pty) => pty,
            Err(e) => {
                return Err(USimpleError::new(
                    1,
                    format!("Failed to open pseudoterminal: {}", e),
                ));
            }
        };

        // Set terminal size
        if isatty {
            let mut ws: Winsize = unsafe { std::mem::zeroed() };
            if unsafe { libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut ws) } == 0 {
                unsafe { libc::ioctl(pty.master.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
            }
        }

        // Configure echo mode for the slave PTY
        if let Some(termios_settings) = termios {
            let mut new_termios = termios_settings.clone();
            match options.echo_mode {
                EchoMode::Always => {
                    new_termios.local_flags |= termios::LocalFlags::ECHO;
                }
                EchoMode::Never => {
                    new_termios.local_flags &= !termios::LocalFlags::ECHO;
                }
                EchoMode::Auto => {
                    // Default behavior - echo enabled for PTY
                    if isatty {
                        // If stdin is a terminal, disable echo to prevent double echo
                        new_termios.local_flags &= !termios::LocalFlags::ECHO;
                    } else {
                        // If stdin is not a terminal, keep echo enabled
                        new_termios.local_flags |= termios::LocalFlags::ECHO;
                    }
                }
            }

            if let Err(e) = termios::tcsetattr(&pty.slave, termios::SetArg::TCSANOW, &new_termios) {
                return Err(USimpleError::new(
                    1,
                    format!("Failed to set terminal attributes: {}", e),
                ));
            }
        }

        // Write start message
        if !options.quiet {
            println!(
                "Script started, file is {}",
                options.typescript_file.display()
            );
        }

        // Record start time
        let start_time = Instant::now();

        // Fork a child process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Close the slave end of the pty in the parent
                let _ = close(pty.slave.as_raw_fd());

                let io_handler_config = IoHandlerConfig {
                    log_format: options.logging_format,
                    flush: options.flush,
                    output_limit: options.output_limit,
                    start_time,
                };

                let log_files = LogFiles {
                    out_file,
                    log_in_file,
                    log_out_file,
                    log_io_file,
                    timing_file,
                };

                // Set up I/O handling
                let result = handle_io(pty.master.as_raw_fd(), child, log_files, io_handler_config);

                // Write end message
                if !options.quiet {
                    println!("Script done, file is {}", options.typescript_file.display());
                }

                // Return exit status if requested
                if options.return_exit_status {
                    match result {
                        Ok(status) => {
                            uucore::error::set_exit_code(status);
                        }
                        Err(e) => {
                            return Err(USimpleError::new(1, format!("Error: {}", e)));
                        }
                    }
                }
            }
            Ok(ForkResult::Child) => {
                // Child process
                // Close the master end of the pty in the Child
                let _ = close(pty.master.as_raw_fd());

                // Make the slave PTY the controlling terminal
                unsafe {
                    libc::setsid();
                    libc::ioctl(pty.slave.as_raw_fd(), libc::TIOCSCTTY as u64, 0);
                }

                // Redirect stdin, stdout, and stderr to the slave PTY
                if let Err(e) = dup2(pty.slave.as_raw_fd(), 0) {
                    eprintln!("Failed to redirect stdin: {}", e);
                    unsafe { libc::_exit(1) };
                }
                if let Err(e) = dup2(pty.slave.as_raw_fd(), 1) {
                    eprintln!("Failed to redirect stdout: {}", e);
                    unsafe { libc::_exit(1) };
                }
                if let Err(e) = dup2(pty.slave.as_raw_fd(), 2) {
                    eprintln!("Failed to redirect stderr: {}", e);
                    unsafe { libc::_exit(1) };
                }

                // Close the slave PTY as it's no longer needed (it's been duplicated)
                let _ = close(pty.slave.as_raw_fd());

                // Execute the shell or command
                let shell = std::env::var("SHELL").unwrap_or_else(|_| String::from("/bin/sh"));

                if let Some(cmd) = options.command {
                    let args = vec!["-c".to_string(), cmd];
                    let c_shell = CString::new(shell.clone()).unwrap();
                    let c_args: Vec<CString> = std::iter::once(CString::new(shell).unwrap())
                        .chain(args.into_iter().map(|s| CString::new(s).unwrap()))
                        .collect();

                    let _ = execvp(&c_shell, &c_args);
                    eprintln!("Failed to execute command: {}", io::Error::last_os_error());
                } else {
                    let c_shell = CString::new(shell.clone()).unwrap();
                    let c_args = vec![CString::new(shell).unwrap()];

                    let _ = execvp(&c_shell, &c_args);
                    eprintln!("Failed to execute shell: {}", io::Error::last_os_error());
                }

                unsafe { libc::_exit(1) };
            }
            Err(e) => {
                return Err(USimpleError::new(1, format!("Fork failed: {}", e)));
            }
        }

        Ok(())
    }

    fn handle_io(
        master_fd: RawFd,
        child_pid: Pid,
        mut log_files: LogFiles,
        config: IoHandlerConfig,
    ) -> Result<i32, String> {
        let mut total_bytes = 0u64;
        let mut last_time = config.start_time;
        let mut buffer = [0u8; 1024];
        let mut stdin_buffer = [0u8; 1024];

        // Set stdin to non-blocking mode
        let stdin_fd = io::stdin().as_raw_fd();
        let mut stdin_flags = unsafe { libc::fcntl(stdin_fd, libc::F_GETFL, 0) };
        if stdin_flags < 0 {
            return Err(format!(
                "Failed to get stdin flags: {}",
                io::Error::last_os_error()
            ));
        }

        stdin_flags |= libc::O_NONBLOCK;
        if unsafe { libc::fcntl(stdin_fd, libc::F_SETFL, stdin_flags) } < 0 {
            return Err(format!(
                "Failed to set stdin flags: {}",
                io::Error::last_os_error()
            ));
        }

        // Set master PTY to non-blocking mode
        let mut master_flags = unsafe { libc::fcntl(master_fd, libc::F_GETFL, 0) };
        if master_flags < 0 {
            return Err(format!(
                "Failed to get master PTY flags: {}",
                io::Error::last_os_error()
            ));
        }

        master_flags |= libc::O_NONBLOCK;
        if unsafe { libc::fcntl(master_fd, libc::F_SETFL, master_flags) } < 0 {
            return Err(format!(
                "Failed to set master PTY flags: {}",
                io::Error::last_os_error()
            ));
        }

        // Prepare for select()
        let mut exit_status = 0;
        let mut child_exited = false;

        while !child_exited {
            let mut read_fds: libc::fd_set = unsafe { std::mem::zeroed() };
            unsafe {
                libc::FD_ZERO(&mut read_fds);
                libc::FD_SET(stdin_fd, &mut read_fds);
                libc::FD_SET(master_fd, &mut read_fds);
            }

            // Wait for data or signals
            let mut tv: libc::timeval = libc::timeval {
                tv_sec: 1,
                tv_usec: 0,
            };

            let select_result = unsafe {
                libc::select(
                    std::cmp::max(stdin_fd, master_fd) + 1,
                    &mut read_fds,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut tv,
                )
            };

            if select_result < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    // Check if child has exited
                    let mut status: libc::c_int = 0;
                    let wait_result =
                        unsafe { libc::waitpid(child_pid.as_raw(), &mut status, libc::WNOHANG) };

                    if wait_result > 0 {
                        child_exited = true;
                        if libc::WIFEXITED(status) {
                            exit_status = libc::WEXITSTATUS(status);
                        } else if libc::WIFSIGNALED(status) {
                            exit_status = 128 + libc::WTERMSIG(status);
                        }
                    }

                    continue;
                } else {
                    return Err(format!("select() failed: {}", err));
                }
            }

            // Check if child has exited
            let mut status: libc::c_int = 0;
            let wait_result =
                unsafe { libc::waitpid(child_pid.as_raw(), &mut status, libc::WNOHANG) };

            if wait_result > 0 {
                child_exited = true;
                if libc::WIFEXITED(status) {
                    exit_status = libc::WEXITSTATUS(status);
                } else if libc::WIFSIGNALED(status) {
                    exit_status = 128 + libc::WTERMSIG(status);
                }
            }

            // Check if we need to flush logs due to SIGUSR1
            if FLUSH_LOGS.swap(false, Ordering::SeqCst) {
                if let Err(e) = log_files.out_file.flush() {
                    eprintln!("Failed to flush output file: {}", e);
                }

                if let Some(ref mut file) = log_files.log_in_file {
                    if let Err(e) = file.flush() {
                        eprintln!("Failed to flush input log file: {}", e);
                    }
                }

                if let Some(ref mut file) = log_files.log_out_file {
                    if let Err(e) = file.flush() {
                        eprintln!("Failed to flush output log file: {}", e);
                    }
                }

                if let Some(ref mut file) = log_files.log_io_file {
                    if let Err(e) = file.flush() {
                        eprintln!("Failed to flush I/O log file: {}", e);
                    }
                }

                if let Some(ref mut file) = log_files.timing_file {
                    if let Err(e) = file.flush() {
                        eprintln!("Failed to flush timing file: {}", e);
                    }
                }
            }

            // Check if stdin has data
            if unsafe { libc::FD_ISSET(stdin_fd, &read_fds) } {
                match unsafe {
                    libc::read(
                        stdin_fd,
                        stdin_buffer.as_mut_ptr() as *mut libc::c_void,
                        stdin_buffer.len(),
                    )
                } {
                    n if n > 0 => {
                        let now = Instant::now();
                        let elapsed = now.duration_since(last_time);
                        last_time = now;

                        // Write to master PTY
                        if let Err(e) = write_all(master_fd, &stdin_buffer[0..n as usize]) {
                            eprintln!("Failed to write to master PTY: {}", e);
                        }

                        // Log input if requested
                        if let Some(ref mut file) = log_files.log_in_file {
                            if let Err(e) = file.write_all(&stdin_buffer[0..n as usize]) {
                                eprintln!("Failed to write to input log file: {}", e);
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush input log file: {}", e);
                                }
                            }
                        }

                        // Log I/O if requested
                        if let Some(ref mut file) = log_files.log_io_file {
                            if let Err(e) = file.write_all(&stdin_buffer[0..n as usize]) {
                                eprintln!("Failed to write to I/O log file: {}", e);
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush I/O log file: {}", e);
                                }
                            }
                        }

                        // Write timing information if requested
                        if let Some(ref mut file) = log_files.timing_file {
                            match config.log_format {
                                LogFormat::Classic => {
                                    if let Err(e) =
                                        writeln!(file, "{:.6} {}", elapsed.as_secs_f64(), n)
                                    {
                                        eprintln!("Failed to write to timing file: {}", e);
                                    }
                                }
                                LogFormat::Advanced => {
                                    if let Err(e) =
                                        writeln!(file, "I {:.6} {}", elapsed.as_secs_f64(), n)
                                    {
                                        eprintln!("Failed to write to timing file: {}", e);
                                    }
                                }
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush timing file: {}", e);
                                }
                            }
                        }
                    }
                    n if n < 0 => {
                        let err = io::Error::last_os_error();
                        if err.kind() != io::ErrorKind::WouldBlock {
                            eprintln!("Failed to read from stdin: {}", err);
                        }
                    }
                    _ => {
                        // EOF on stdin, but we continue as the child might still produce output
                    }
                }
            }

            // Check if master PTY has data
            if unsafe { libc::FD_ISSET(master_fd, &read_fds) } {
                match unsafe {
                    libc::read(
                        master_fd,
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        buffer.len(),
                    )
                } {
                    n if n > 0 => {
                        let now = Instant::now();
                        let elapsed = now.duration_since(last_time);
                        last_time = now;

                        // Write to stdout
                        if let Err(e) = io::stdout().write_all(&buffer[0..n as usize]) {
                            eprintln!("Failed to write to stdout: {}", e);
                        }

                        // Write to typescript file
                        if let Err(e) = log_files.out_file.write_all(&buffer[0..n as usize]) {
                            eprintln!("Failed to write to typescript file: {}", e);
                        }

                        if config.flush {
                            if let Err(e) = log_files.out_file.flush() {
                                eprintln!("Failed to flush typescript file: {}", e);
                            }
                        }

                        // Log output if requested
                        if let Some(ref mut file) = log_files.log_out_file {
                            if let Err(e) = file.write_all(&buffer[0..n as usize]) {
                                eprintln!("Failed to write to output log file: {}", e);
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush output log file: {}", e);
                                }
                            }
                        }

                        // Log I/O if requested
                        if let Some(ref mut file) = log_files.log_io_file {
                            if let Err(e) = file.write_all(&buffer[0..n as usize]) {
                                eprintln!("Failed to write to I/O log file: {}", e);
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush I/O log file: {}", e);
                                }
                            }
                        }

                        // Write timing information if requested
                        if let Some(ref mut file) = log_files.timing_file {
                            match config.log_format {
                                LogFormat::Classic => {
                                    if let Err(e) =
                                        writeln!(file, "O {:.6} {}\n", elapsed.as_secs_f64(), n)
                                    {
                                        eprintln!("Failed to write to timing file: {}", e);
                                    }
                                }
                                LogFormat::Advanced => {
                                    if let Err(e) =
                                        writeln!(file, "O {:.6} {}\n", elapsed.as_secs_f64(), n)
                                    {
                                        eprintln!("Failed to write to timing file: {}", e);
                                    }
                                }
                            }

                            if config.flush {
                                if let Err(e) = file.flush() {
                                    eprintln!("Failed to flush timing file: {}", e);
                                }
                            }
                        }

                        // Update total bytes and check output limit
                        total_bytes += n as u64;
                        if let Some(limit) = config.output_limit {
                            if total_bytes >= limit {
                                // Kill the child process
                                unsafe { libc::kill(child_pid.as_raw(), libc::SIGTERM) };
                                eprintln!("Output limit reached ({} bytes), terminating.", limit);
                                break;
                            }
                        }
                    }
                    n if n < 0 => {
                        let err = io::Error::last_os_error();
                        if err.kind() != io::ErrorKind::WouldBlock {
                            eprintln!("Failed to read from master PTY: {}", err);
                            break;
                        }
                    }
                    _ => {
                        // EOF on master PTY, child has closed its stdout
                        break;
                    }
                }
            }
        }

        // Restore stdin flags
        stdin_flags &= !libc::O_NONBLOCK;
        if unsafe { libc::fcntl(stdin_fd, libc::F_SETFL, stdin_flags) } < 0 {
            eprintln!(
                "Warning: Failed to restore stdin flags: {}",
                io::Error::last_os_error()
            );
        }

        Ok(exit_status)
    }

    fn write_all(fd: RawFd, buf: &[u8]) -> io::Result<()> {
        let mut remaining = buf;
        while !remaining.is_empty() {
            match unsafe {
                libc::write(
                    fd,
                    remaining.as_ptr() as *const libc::c_void,
                    remaining.len(),
                )
            } {
                n if n > 0 => {
                    remaining = &remaining[n as usize..];
                }
                n if n < 0 => {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::WouldBlock
                        || err.kind() == io::ErrorKind::Interrupted
                    {
                        continue;
                    }
                    return Err(err);
                }
                _ => {
                    return Err(io::Error::new(io::ErrorKind::WriteZero, "write returned 0"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(target_family = "unix"))]
mod unix {
    use std::path::PathBuf;
    use uucore::error::UResult;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum LogFormat {
        Classic,
        Advanced,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum EchoMode {
        Always,
        Never,
        Auto,
    }

    pub struct ScriptOptions {
        pub typescript_file: PathBuf,
        pub append: bool,
        pub command: Option<String>,
        pub echo_mode: EchoMode,
        pub return_exit_status: bool,
        pub flush: bool,
        pub force: bool,
        pub log_io_file: Option<PathBuf>,
        pub log_in_file: Option<PathBuf>,
        pub log_out_file: Option<PathBuf>,
        pub log_timing_file: Option<PathBuf>,
        pub logging_format: LogFormat,
        pub output_limit: Option<u64>,
        pub quiet: bool,
    }

    pub fn parse_size(_size_str: &str) -> Result<u64, String> {
        Err("Size parsing not supported on this platform".to_string())
    }

    pub fn run_script(_options: ScriptOptions) -> UResult<()> {
        Err(uucore::error::USimpleError::new(
            1,
            "`script` is unavailable on non-UNIX-like platforms.",
        ))
    }
}

#[cfg(target_family = "unix")]
use unix::*;

#[cfg(target_family = "unix")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let typescript_file = matches
        .get_one::<String>("FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("typescript"));

    let echo_mode = match matches.get_one::<String>("echo").map(|s| s.as_str()) {
        Some("always") => EchoMode::Always,
        Some("never") => EchoMode::Never,
        _ => EchoMode::Auto,
    };

    let logging_format = match matches.get_one::<String>("FORMAT").map(|s| s.as_str()) {
        Some("advanced") => LogFormat::Advanced,
        _ => LogFormat::Classic,
    };

    let output_limit = matches.get_one::<String>("output-limit").map(|s| {
        parse_size(s).unwrap_or_else(|e| {
            eprintln!("script: {}", e);
            std::process::exit(1);
        })
    });

    let options = ScriptOptions {
        typescript_file,
        append: matches.get_flag("append"),
        command: matches.get_one::<String>("command").cloned(),
        echo_mode,
        return_exit_status: matches.get_flag("return"),
        flush: matches.get_flag("flush"),
        force: matches.get_flag("force"),
        log_io_file: matches.get_one::<String>("log-io").map(PathBuf::from),
        log_in_file: matches.get_one::<String>("log-in").map(PathBuf::from),
        log_out_file: matches.get_one::<String>("log-out").map(PathBuf::from),
        log_timing_file: matches.get_one::<String>("log-timing").map(PathBuf::from),
        logging_format,
        output_limit,
        quiet: matches.get_flag("quiet"),
    };

    // Handle deprecated -t option
    if matches.contains_id("timing") {
        let timing_file = matches.get_one::<String>("timing").cloned();
        if options.log_timing_file.is_none() {
            // Only use -t if -T is not specified
            if timing_file.is_some() {
                eprintln!("script: warning: -t/--timing option is deprecated, use -T/--log-timing instead");
            }
        }
    }

    run_script(options)
}

#[cfg(not(target_family = "unix"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`script` is unavailable on non-UNIX-like platforms.",
    ))
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("FILE")
                .help("File to save the output to (default: typescript)")
                .index(1)
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("append")
                .short('a')
                .long("append")
                .help("Append the output to file or to typescript")
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool()),
        )
        .arg(
            Arg::new("command")
                .short('c')
                .long("command")
                .help("Run the command rather than an interactive shell")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("echo")
                .short('E')
                .long("echo")
                .help("Set echo mode (always, never, auto)")
                .value_parser(["always", "never", "auto"])
                .default_value("auto"),
        )
        .arg(
            Arg::new("return")
                .short('e')
                .long("return")
                .help("Return the exit status of the child process")
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool()),
        )
        .arg(
            Arg::new("flush")
                .short('f')
                .long("flush")
                .help("Flush output after each write")
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool()),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .help("Allow the default output file typescript to be a hard or symbolic link")
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool()),
        )
        .arg(
            Arg::new("log-io")
                .short('B')
                .long("log-io")
                .help("Log input and output to the same file")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("log-in")
                .short('I')
                .long("log-in")
                .help("Log input to the file")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("log-out")
                .short('O')
                .long("log-out")
                .help("Log output to the file")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("log-timing")
                .short('T')
                .long("log-timing")
                .help("Log timing information to the file")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("logging-format")
                .short('m')
                .long("logging-format")
                .help("Force use of advanced or classic timing log format")
                .value_parser(["classic", "advanced"])
                .value_name("FORMAT"),
        )
        .arg(
            Arg::new("output-limit")
                .short('o')
                .long("output-limit")
                .help("Limit the size of the typescript and timing files")
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Be quiet (do not write start and done messages)")
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool()),
        )
        .arg(
            Arg::new("timing")
                .short('t')
                .long("timing")
                .help("Output timing data to standard error, or to file when given (deprecated)")
                .value_parser(ValueParser::string())
                .num_args(0..=1),
        )
}
