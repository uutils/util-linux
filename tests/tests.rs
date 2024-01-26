// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
#[macro_use]
mod common;

#[cfg(feature = "lscpu")]
#[path = "by-util/test_lscpu.rs"]
mod test_lscpu;

#[cfg(feature = "pwdx")]
#[path = "by-util/test_pwdx.rs"]
mod test_pwdx;

#[cfg(feature = "mountpoint")]
#[path = "by-util/test_mountpoint.rs"]
mod test_mountpoint;

#[cfg(feature = "renice")]
#[path = "by-util/test_renice.rs"]
mod test_renice;
