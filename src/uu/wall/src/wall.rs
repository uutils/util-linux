use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(unix)]
use uucore::error::USimpleError;

const ABOUT: &str = help_about!("wall.md");
const USAGE: &str = help_usage!("wall.md");

#[cfg(unix)]
mod unix {
    use super::{UResult, USimpleError};

    use chrono::{DateTime, Local};
    use libc::{c_char, gid_t};
    use std::{
        ffi::{CStr, CString},
        fmt::Write as fw,
        fs::OpenOptions,
        io::{BufRead, BufReader, Read, Write},
        str::FromStr,
        sync::{mpsc, Arc},
        time::{Duration, SystemTime},
    };
    use unicode_width::UnicodeWidthChar;

    const TERM_WIDTH: usize = 79;
    const BLANK: &str = unsafe { str::from_utf8_unchecked(&[b' '; TERM_WIDTH]) };
    fn blank(s: &mut String) {
        *s += BLANK;
        *s += "\r\n";
    }

    // go through user entries and print to each tty once.
    // if group is specified, only print to memebers of the group.
    pub fn wall<R: Read>(
        input: R,
        group: Option<gid_t>,
        timeout: Option<&u64>,
        print_banner: bool,
    ) {
        let msg = makemsg(input, print_banner);
        let mut seen_ttys = Vec::with_capacity(16);
        loop {
            // get next user entry and check it is valid
            let entry = unsafe {
                let utmpptr = libc::getutxent();
                if utmpptr.is_null() {
                    break;
                }
                &*utmpptr
            };

            if entry.ut_user[0] == 0 || entry.ut_type != libc::USER_PROCESS {
                continue;
            }

            // make sure device is valid
            let first = entry.ut_line[0].cast_unsigned();
            if first == 0 || first == b':' {
                continue;
            }

            // check group membership
            if let Some(gid) = group {
                if !is_gr_member(&entry.ut_user, gid) {
                    continue;
                }
            }

            // get tty
            let tty = unsafe {
                let len = entry
                    .ut_line
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.ut_line.len());

                let bytes = std::slice::from_raw_parts(entry.ut_line.as_ptr().cast(), len);
                str::from_utf8_unchecked(bytes).to_owned()
            };

            // output message to device
            if !seen_ttys.contains(&tty) {
                if let Err(e) = ttymsg(&tty, msg.clone(), timeout) {
                    eprintln!("warn ({tty:?}): {e}");
                }
                seen_ttys.push(tty);
            }
        }
        unsafe { libc::endutxent() };
    }

    // Create the banner and sanitise input
    fn makemsg<R: Read>(input: R, print_banner: bool) -> Arc<String> {
        let mut buf = String::with_capacity(256);
        if print_banner {
            let hostname = unsafe {
                let mut buf = [0; 256];
                let ret = libc::gethostname(buf.as_mut_ptr(), buf.len());
                if ret == 0 {
                    CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
                } else {
                    "unknown".to_string()
                }
            };

            let whom = unsafe {
                let ruid = libc::getuid();
                let pw = libc::getpwuid(ruid);
                if !pw.is_null() && !(*pw).pw_name.is_null() {
                    CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
                } else {
                    eprintln!("cannot get passwd uid");
                    "<someone>".to_string()
                }
            };

            let whereat = unsafe {
                let tty_ptr = libc::ttyname(libc::STDOUT_FILENO);
                if tty_ptr.is_null() {
                    "somewhere".to_string()
                } else {
                    let s = CStr::from_ptr(tty_ptr).to_string_lossy();
                    s.strip_prefix("/dev/").unwrap_or(&s).to_string()
                }
            };

            let date = DateTime::<Local>::from(SystemTime::now()).format("%a %b %e %T %Y");
            let banner = format!("Broadcast message from {whom}@{hostname} ({whereat}) ({date}):");

            blank(&mut buf);
            buf += &banner;
            buf.extend(std::iter::repeat_n(
                ' ',
                TERM_WIDTH.saturating_sub(banner.len()),
            ));
            buf += "\x07\x07\r\n";
        }

        // we put a blank box around our input
        blank(&mut buf);
        let mut reader = BufReader::new(input).lines();
        while let Some(Ok(line)) = reader.next() {
            buf += &sanitise_line(&line);
        }
        blank(&mut buf);

        Arc::new(buf)
    }

    // this function does two things:
    // - wraps lines by TERM_WIDTH
    // - escapes control characters
    fn sanitise_line(line: &str) -> String {
        let mut buf = String::with_capacity(line.len());
        let mut col = 0;

        for ch in line.chars() {
            // sanitise character
            match ch {
                '\x07' => buf.push(ch),
                '\t' => {
                    buf.push(ch);
                    col += 7 - (col % 8);
                }
                _ if ch.is_ascii_control() => {
                    buf.push('^');
                    buf.push((ch as u8 ^ 0x40) as char);
                    col += 2;
                }
                _ if (0x80_u8..=0x9F).contains(&(ch as u8)) => {
                    let _ = write!(buf, "\\x{:02X}", ch as u8);
                    col += 4;
                }
                _ if ch.is_control() => {
                    let _ = write!(buf, "\\u{:04X}", ch as u32);
                    col += 6;
                }
                _ => {
                    buf.push(ch);
                    col += ch.width_cjk().unwrap_or_default();
                }
            }

            // wrap line
            if col >= TERM_WIDTH {
                buf += "\r\n";
                col = 0;
            }
        }

        // fill rest of line with spaces
        buf.extend(std::iter::repeat_n(' ', TERM_WIDTH.saturating_sub(col)));
        buf + "\r\n"
    }

    // Determine if user is in specified group
    #[allow(clippy::cast_sign_loss)]
    fn is_gr_member(user: &[c_char], gid: gid_t) -> bool {
        // make sure user exists in database
        let pw = unsafe { libc::getpwnam(user.as_ptr()) };
        if pw.is_null() {
            return false;
        }

        // if so, check if primary group matches
        let group = unsafe { (*pw).pw_gid };
        if gid == group {
            return true;
        }

        // on macos, getgrouplist takes c_int as its group argument
        #[cfg(target_os = "macos")]
        let group = group.cast_signed();

        // otherwise check gid is in list of supplementary groups user belongs to
        let mut ngroups = 16;
        let mut groups = vec![0; ngroups as usize];
        while unsafe {
            libc::getgrouplist(user.as_ptr(), group, groups.as_mut_ptr(), &raw mut ngroups)
        } == -1
        {
            // ret -1 means buffer was too small so we resize
            // according to the returned ngroups value
            groups.resize(ngroups as usize, 0);
        }

        #[cfg(target_os = "macos")]
        let gid = gid.cast_signed();
        groups.contains(&gid)
    }

    // Try to get corresponding group gid.
    pub fn get_group_gid(group: &String) -> UResult<gid_t> {
        // first we try as a group name
        let Ok(cname) = CString::from_str(group) else {
            return Err(USimpleError::new(1, "invalid group argument"));
        };

        let gr = unsafe { libc::getgrnam(cname.as_ptr()) };
        if !gr.is_null() {
            return Ok(unsafe { (*gr).gr_gid });
        }

        // otherwise, try as literal gid
        let Ok(gid) = group.parse::<gid_t>() else {
            return Err(USimpleError::new(1, "invalid group argument"));
        };
        if unsafe { libc::getgrgid(gid) }.is_null() {
            return Err(USimpleError::new(1, format!("{group}: unknown gid")));
        }
        Ok(gid)
    }

    // Write to the tty device
    fn ttymsg(tty: &str, msg: Arc<String>, timeout: Option<&u64>) -> Result<(), &'static str> {
        let (tx, rx) = mpsc::channel();
        let device = String::from("/dev/") + tty;

        // spawn thread to write to device
        std::thread::spawn(move || {
            let r = match OpenOptions::new().write(true).open(&device) {
                Ok(mut f) => f.write_all(msg.as_bytes()).map_err(|_| "write failed"),
                Err(_) => Err("open failed"),
            };
            let _ = tx.send(r);
        });

        // wait with timeout if specified, otherwise block
        if let Some(&t) = timeout {
            rx.recv_timeout(Duration::from_secs(t))
                .map_err(|_| "write timeout")?
        } else {
            rx.recv().map_err(|_| "channel closed")?
        }
    }
}

#[must_use]
pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("input")
                .value_name("<file> | <message>")
                .help("file to read or literal message")
                .num_args(1..)
                .index(1),
        )
        .arg(
            Arg::new("group")
                .short('g')
                .long("group")
                .help("only send mesage to group"),
        )
        .arg(
            Arg::new("nobanner")
                .short('n')
                .long("nobanner")
                .action(ArgAction::SetTrue)
                .help("do not print banner, works only for root"),
        )
        .arg(
            Arg::new("timeout")
                .short('t')
                .long("timeout")
                .value_parser(clap::value_parser!(u64))
                .help("write timeout in seconds"),
        )
}

#[cfg(not(unix))]
#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    Err(uucore::error::USimpleError::new(
        1,
        "`wall` is available only on Unix.",
    ))
}

#[cfg(unix)]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    use std::fs::File;
    use std::path::Path;

    let args = uu_app().try_get_matches_from_mut(args)?;

    // clap will reject non-integer values, so we just need to reject 0
    let timeout = args.get_one::<u64>("timeout");
    if timeout == Some(&0) {
        return Err(USimpleError::new(1, "invalid timeout argument: 0"));
    }

    // get nobanner flag and check if user is root
    let flag = args.get_flag("nobanner");
    let print_banner = if flag && unsafe { libc::geteuid() } != 0 {
        eprintln!("wall: --nobanner is available only for root");
        true
    } else {
        !flag
    };

    // if group exists, map to corresponding gid
    let group = args
        .get_one::<String>("group")
        .map(unix::get_group_gid)
        .transpose()?;

    // If we have a single input arg and it exists on disk, treat as a file.
    // If either is false, assume it is a literal string.
    // If no input given, use stdin.
    if let Some(v) = args.get_many::<String>("input") {
        let vals: Vec<&str> = v.map(String::as_str).collect();

        let fname = vals
            .first()
            .expect("clap guarantees at least 1 value for input");

        let p = Path::new(fname);
        if vals.len() == 1 && p.exists() {
            // When we are not root, but suid or sgid, refuse to read files
            // (e.g. device files) that the user may not have access to.
            // After all, our invoker can easily do "wall < file" instead of "wall file".
            unsafe {
                let uid = libc::getuid();
                if uid > 0 && (uid != libc::geteuid() || libc::getgid() != libc::getegid()) {
                    return Err(USimpleError::new(
                        1,
                        format!("will not read {fname} - use stdin"),
                    ));
                }
            }

            let Ok(f) = File::open(p) else {
                return Err(USimpleError::new(1, format!("cannot open {fname}")));
            };

            unix::wall(f, group, timeout, print_banner);
        } else {
            let mut s = vals.as_slice().join(" ");
            s.push('\n');
            unix::wall(s.as_bytes(), group, timeout, print_banner);
        }
    } else {
        unix::wall(std::io::stdin(), group, timeout, print_banner);
    }

    Ok(())
}
