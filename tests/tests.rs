// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
#[macro_use]
mod common;

#[cfg(feature = "lscpu")]
#[path = "by-util/test_lscpu.rs"]
mod test_lscpu;

#[cfg(feature = "lsmem")]
#[path = "by-util/test_lsmem.rs"]
mod test_lsmem;

#[cfg(feature = "mountpoint")]
#[path = "by-util/test_mountpoint.rs"]
mod test_mountpoint;

#[cfg(feature = "ctrlaltdel")]
#[path = "by-util/test_ctrlaltdel.rs"]
mod test_ctrlaltdel;

#[cfg(feature = "rev")]
#[path = "by-util/test_rev.rs"]
mod test_rev;

#[cfg(feature = "setsid")]
#[path = "by-util/test_setsid.rs"]
mod test_setsid;

#[cfg(feature = "last")]
#[path = "by-util/test_last.rs"]
mod test_last;
