// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: pwdx <pid>");
        process::exit(1);
    }

    let pid = &args[1];
    let cwd_link = format!("/proc/{}/cwd", pid);

    match fs::read_link(Path::new(&cwd_link)) {
        Ok(path) => println!("{}: {}", pid, path.display()),
        Err(e) => {
            eprintln!("pwdx: failed to read link for PID {}: {}", pid, e);
            process::exit(1);
        }
    }
}
