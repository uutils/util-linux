// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS},
    NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_UNICAST,
        IP_ADAPTER_ADDRESSES_LH,
    },
    Networking::WinSock::AF_UNSPEC,
};

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};

#[cfg(all(target_family = "unix", not(target_os = "redox")))]
use nix::ifaddrs::getifaddrs;
use uucore::{
    error::{UResult, USimpleError},
    format_usage, help_about, help_usage,
};
use uuid::Uuid;

const ABOUT: &str = help_about!("uuidgen.md");
const USAGE: &str = help_usage!("uuidgen.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = uu_app().try_get_matches_from_mut(args)?;

    let md5 = args.get_flag(options::MD5);
    let sha1 = args.get_flag(options::SHA1);

    let namespace = args.get_one(options::NAMESPACE);
    let name: Option<&String> = args.get_one(options::NAME);

    // https://github.com/clap-rs/clap/issues/1537
    if !(md5 || sha1) && (namespace.is_some() || name.is_some()) {
        return Err(USimpleError::new(
            1,
            "--namespace and --name arguments require either --md5 or --sha1",
        ));
    }

    if args.get_flag(options::TIME) {
        let node_id = get_node_id().unwrap_or_else(|| {
            let mut default: [u8; 6] = rand::random();
            default[0] |= 0x01;
            default
        });
        println!("{:?}", Uuid::now_v1(&node_id));
    } else if md5 || sha1 {
        let f = if md5 { Uuid::new_v3 } else { Uuid::new_v5 };

        println!(
            "{:?}",
            f(
                namespace.expect("namespace arg to be set"),
                name.expect("name to be set").as_bytes()
            )
        );
    } else {
        // Random is the default
        println!("{}", Uuid::new_v4());
    }

    Ok(())
}

pub fn uu_app() -> Command {
    let all_uuid_types = [options::RANDOM, options::TIME, options::MD5, options::SHA1];

    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::RANDOM)
                .short('r')
                .long(options::RANDOM)
                .action(ArgAction::SetTrue)
                .help("generate random UUID (v4)"),
        )
        .arg(
            Arg::new(options::TIME)
                .short('t')
                .long(options::TIME)
                .action(ArgAction::SetTrue)
                .help("generate time UUID (v1)"),
        )
        .arg(
            Arg::new(options::NAMESPACE)
                .short('n')
                .long(options::NAMESPACE)
                .action(ArgAction::Set)
                .value_parser(namespace_from_str)
                .help("namespace for md5/sha1 - one of: @dns @url @oid @x500"),
        )
        .arg(
            Arg::new(options::NAME)
                .short('N')
                .long(options::NAME)
                .action(ArgAction::Set)
                .help("name for md5/sha1"),
        )
        .arg(
            Arg::new(options::MD5)
                .short('m')
                .long(options::MD5)
                .action(ArgAction::SetTrue)
                .requires_all([options::NAMESPACE, options::NAME])
                .help("generate md5 UUID (v3)"),
        )
        .arg(
            Arg::new(options::SHA1)
                .short('s')
                .long(options::SHA1)
                .action(ArgAction::SetTrue)
                .requires_all([options::NAMESPACE, options::NAME])
                .help("generate sha1 UUID (v5)"),
        )
        .group(ArgGroup::new("mode").args(all_uuid_types).multiple(false))
}

fn namespace_from_str(s: &str) -> Result<Uuid, USimpleError> {
    match s {
        "@dns" => Ok(Uuid::NAMESPACE_DNS),
        "@url" => Ok(Uuid::NAMESPACE_URL),
        "@oid" => Ok(Uuid::NAMESPACE_OID),
        "@x500" => Ok(Uuid::NAMESPACE_X500),
        _ => Err(USimpleError {
            code: 1,
            message: format!("Invalid namespace {}.", s),
        }),
    }
}

mod options {
    pub const RANDOM: &str = "random";
    pub const TIME: &str = "time";
    pub const MD5: &str = "md5";
    pub const SHA1: &str = "sha1";
    pub const NAMESPACE: &str = "namespace";
    pub const NAME: &str = "name";
}

#[cfg(target_os = "windows")]
fn get_node_id() -> Option<[u8; 6]> {
    unsafe {
        // Skip everything we can - we are only interested in PhysicalAddress
        let flags = GAA_FLAG_SKIP_UNICAST
            | GAA_FLAG_SKIP_ANYCAST
            | GAA_FLAG_SKIP_MULTICAST
            | GAA_FLAG_SKIP_DNS_SERVER
            | GAA_FLAG_SKIP_FRIENDLY_NAME;

        let mut size = 0;
        let ret = GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, None, &mut size);
        if ret != ERROR_BUFFER_OVERFLOW.0 {
            return None;
        }

        let mut buf = vec![0u8; size as usize];
        let ret = GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            flags,
            None,
            Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
            &mut size,
        );
        if ret != ERROR_SUCCESS.0 {
            return None;
        }

        // SAFETY: GetAdaptersAddresses returns ERROR_NO_DATA error if it's zero len
        let mut adapter_ptr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !adapter_ptr.is_null() {
            let adapter = adapter_ptr.read();

            if adapter.PhysicalAddressLength == 6 {
                return Some(adapter.PhysicalAddress[0..6].try_into().unwrap());
            }

            adapter_ptr = adapter.Next;
        }
    }
    None
}

#[cfg(all(target_family = "unix", not(target_os = "redox")))]
fn get_node_id() -> Option<[u8; 6]> {
    getifaddrs().ok().and_then(|iflist| {
        iflist
            .filter_map(|intf| intf.address?.as_link_addr()?.addr())
            .find(|mac| mac.iter().any(|x| *x != 0))
    })
}

#[cfg(not(any(
    target_os = "windows",
    all(target_family = "unix", not(target_os = "redox"))
)))]
fn get_node_id() -> Option<[u8; 6]> {
    None
}
