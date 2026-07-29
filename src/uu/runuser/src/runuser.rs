// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
use uucore::error::{UResult, USimpleError};
use uucore::{format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("runuser.md");
const USAGE: &str = help_usage!("runuser.md");

#[cfg(target_os = "linux")]
mod linux {
    use nix::unistd::{setgid, setgroups, setsid, setuid, Gid, Uid};
    use std::env::var;
    use std::error::Error;
    use std::fs::read_to_string;
    use std::io::Error as IOError;
    use std::process::Command as RunCommand;
    use uucore::error::{UResult, USimpleError};

    pub fn get_conf(filename: &str, query: &str) -> Result<Option<Vec<String>>, IOError> {
        let file = read_to_string(filename)?;
        let line = file
            .lines()
            .find(|line| line.starts_with(&format!("{}:", query)));

        match line {
            Some(line) => {
                let parts: Vec<String> = line.split(':').map(|s| s.to_string()).collect();

                Ok(Some(parts))
            }
            None => Ok(None),
        }
    }

    pub struct UserEntry {
        pub uid: u32,
        pub gid: u32,
        pub home: String,
        pub shell: String,
    }

    pub fn get_user_info(username: &str) -> Result<Option<UserEntry>, Box<dyn Error>> {
        let info = get_conf("/etc/passwd", username)?;

        match info {
            Some(parts) => {
                if parts.len() < 7 {
                    return Err("Invalid passwd format".into());
                }
                let [_, _, ref uid_str, ref gid_str, _, ref home_dir, ref shell_path] = parts[0..7]
                else {
                    unreachable!()
                };
                let uid = uid_str.parse::<u32>()?;
                let gid = gid_str.parse::<u32>()?;

                Ok(Some(UserEntry {
                    uid,
                    gid,
                    home: home_dir.to_string(),
                    shell: shell_path.to_string(),
                }))
            }
            None => Ok(None),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn get_group_info(groupname: &str) -> Result<Option<u32>, Box<dyn Error>> {
        let info = get_conf("/etc/group", groupname)?;

        match info {
            Some(parts) => {
                let gid = parts.get(2).ok_or("Invalid group format")?.parse::<u32>()?;

                Ok(Some(gid))
            }
            None => Ok(None),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_ids(uid: u32, gid: u32, supp_gids: &Vec<Gid>) -> UResult<()> {
        setgroups(supp_gids.as_slice())
            .map_err(|e| USimpleError::new(1, format!("Failed to set supp Gid: {}", e)))?;
        setgid(Gid::from_raw(gid))
            .map_err(|e| USimpleError::new(1, format!("Failed to set Gid to {}: {}", gid, e)))?;
        setuid(Uid::from_raw(uid))
            .map_err(|e| USimpleError::new(1, format!("Failed to set Uid to {}: {}", uid, e)))?;

        Ok(())
    }

    pub fn prepare_env(
        cmd: &mut RunCommand,
        home_dir: &str,
        shell_path: &str,
        username: &str,
        preserve_env: bool,
        whitelist_env: &Vec<String>,
        is_login: bool,
    ) {
        const ROOT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/sbin:/bin:/usr/sbin:/usr/bin";

        if is_login {
            if preserve_env {
                println!("--preserve-environment is ignored in case `--login` is enabled");
            }
            cmd.env_clear();
        }
        if is_login || !preserve_env {
            match username {
                "root" => {
                    cmd.env("PATH", ROOT_PATH);
                }
                _ => {
                    cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
                }
            }
            cmd.env("HOME", home_dir);
            cmd.env("USER", username);
            cmd.env("LOGNAME", username);
            cmd.env("SHELL", shell_path);
        }
        for item in whitelist_env {
            if !["HOME", "USER", "LOGNAME", "SHELL", "PATH"].contains(&item.as_str()) {
                if let Ok(val) = var(item) {
                    cmd.env(item, val);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub fn run(cmd: &mut RunCommand) -> UResult<i32> {
        let status = cmd
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => USimpleError::new(127, "command was not found"),
                e => USimpleError::new(126, format!("Failed to spawn for process: {}", e)),
            })?
            .wait()
            .map_err(|e| USimpleError::new(126, format!("Failed to wait for process: {}", e)))?;

        Ok(status.code().unwrap_or(0))
    }

    #[cfg(target_os = "linux")]
    pub fn sep_session() -> UResult<()> {
        setsid().map_err(|e| USimpleError::new(1, format!("Failed to set Sid: {}", e)))?;

        Ok(())
    }
}

#[cfg(target_os = "linux")]
use linux::*;

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    use nix::unistd::Gid;
    use std::process::Command as RunCommand;
    let matches = uu_app().try_get_matches_from(args)?;
    let mut is_login = false;
    let mut command_args = Vec::new();
    if *matches.get_one::<bool>("login").unwrap_or(&false) {
        command_args.push("-l".to_string());
        is_login = true;
    }
    if *matches.get_one::<bool>("fast").unwrap_or(&false) {
        command_args.push("-f".to_string());
    }
    if matches.contains_id("cmd") {
        let command = match matches.get_one::<String>("session_command") {
            Some(cmd) => cmd,
            None => {
                sep_session()?;
                matches.get_one::<String>("command").unwrap()
            }
        };
        command_args.push("-c".to_string());
        command_args.push(command.to_string());
    }
    let supp_gids = if let Some(supp_groups) = matches.get_many::<String>("supp_group") {
        let mut supp_gids = Vec::new();

        for supp_group in supp_groups {
            supp_gids.push(Gid::from_raw(
                get_group_info(supp_group)
                    .map_err(|e| {
                        USimpleError::new(1, format!("Failed to get supp group info: {}", e))
                    })?
                    .ok_or(USimpleError::new(1, "Supp group doesn't exist"))?,
            ));
        }

        Some(supp_gids)
    } else {
        None
    };
    let overwritten_gid = if let Some(overwritten_group) = matches.get_one::<String>("group") {
        Some(
            get_group_info(overwritten_group)
                .map_err(|e| USimpleError::new(1, format!("Failed to get group info: {}", e)))?
                .ok_or(USimpleError::new(1, "Group doesn't exist"))?,
        )
    } else {
        None
    };
    let rest: Vec<String> = matches
        .get_many::<String>("rest")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .to_vec();
    let (username, path, command_args) = match matches.get_one::<String>("user") {
        Some(username) => match matches.contains_id("cmd") {
            true => (
                Some(username.to_string()),
                matches.get_one::<String>("shell").cloned(),
                command_args,
            ),
            false => {
                if rest.is_empty() {
                    return Err(USimpleError::new(1, "Incorrect usage"));
                }
                let path = rest[0].clone();
                let args = rest[1..].to_vec();

                (Some(username.to_string()), Some(path), args)
            }
        },
        None => {
            let mut rest = rest.clone();
            let mut username: Option<String> = None;

            if let Some(arg) = rest.first() {
                if arg == "-" {
                    is_login = true;
                    command_args.push("-l".to_string());
                    rest.remove(0);
                }
            }
            if let Some(arg) = rest.first() {
                username = Some(arg.to_string());
                rest.remove(0);
            }
            match matches.contains_id("cmd") {
                true => (
                    username,
                    matches.get_one::<String>("shell").cloned(),
                    command_args,
                ),
                false => {
                    command_args.extend(rest.clone());

                    (username, None, command_args)
                }
            }
        }
    };
    let user_info = get_user_info(&username.clone().unwrap_or("root".to_string()))
        .map_err(|e| USimpleError::new(1, format!("Failed to get user info: {}", e)))?
        .ok_or(USimpleError::new(1, "User doesn't exist"))?;

    set_ids(
        user_info.uid,
        overwritten_gid.unwrap_or(user_info.gid),
        &supp_gids.unwrap_or(Vec::new()),
    )?;

    let mut cmd = RunCommand::new(path.unwrap_or(user_info.shell.clone()));
    cmd.args(&command_args);
    prepare_env(
        &mut cmd,
        &user_info.home,
        &user_info.shell,
        &username.unwrap_or("root".to_string()),
        *matches.get_one::<bool>("preserve_env").unwrap_or(&false),
        &matches
            .get_many::<String>("whitelist_env")
            .unwrap_or_default()
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .to_vec(),
        is_login,
    );
    let status = run(&mut cmd)?;

    if status != 0 {
        return Err(USimpleError::new(
            status + 128,
            format!("Process exited with status code {}", status),
        ));
    }

    Ok(())
}

// TODO: I haven't found if this code can actually work well
//       on non-linux unix operating systems.
#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`runuser` is only available on linux",
    ))
}

pub fn uu_app() -> Command {
    // TODO: to --pty yet
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .arg(Arg::new("user").short('u').long("user").value_name("user"))
        .arg(
            Arg::new("preserve_env")
                .short('p')
                .long("preserve-environment")
                .visible_short_alias('m')
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("whitelist_env")
                .short('w')
                .long("whitelist-environment")
                .value_name("list")
                .num_args(0..),
        )
        .arg(
            Arg::new("group")
                .short('g')
                .long("group")
                .value_name("group"),
        )
        .arg(
            Arg::new("supp_group")
                .short('F')
                .long("supp-group")
                .value_name("supp-group"),
        )
        .arg(
            Arg::new("login")
                .short('l')
                .long("login")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("command")
                .short('c')
                .long("command")
                .value_name("command"),
        )
        .arg(
            Arg::new("session_command")
                .long("session-command")
                .value_name("command"),
        )
        .group(ArgGroup::new("cmd").args(["command", "session_command"]))
        .arg(
            Arg::new("fast")
                .short('f')
                .long("fast")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("shell")
                .short('s')
                .long("shell")
                .value_name("shell"),
        )
        .arg(
            Arg::new("rest")
                .num_args(0..)
                .hide(true)
                .allow_hyphen_values(true)
                .trailing_var_arg(true),
        )
}
