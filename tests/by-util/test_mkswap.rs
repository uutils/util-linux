// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
mod linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_invalid_path() {
        new_ucmd!()
            .arg("-d")
            .arg("/foo/bar/baz")
            .fails()
            .code_is(2)
            .stderr_contains("failed to open /foo/bar/baz: No such file or directory");
    }

    #[test]
    fn test_directory_err() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("foo");
        ucmd.arg("-d")
            .arg("foo")
            .fails()
            .code_is(2)
            .stderr_contains("failed to open foo: Is a directory");
    }

    #[test]
    fn test_invalid_arg() {
        new_ucmd!().arg("foo").fails().code_is(1);
    }
    #[test]
    fn test_empty_args() {
        new_ucmd!().fails().code_is(2).stderr_contains("Usage:");
    }

    #[test]
    fn test_empty_file() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("empty");
        ucmd.arg("-d")
            .arg("empty")
            .fails()
            .stderr_contains("swap space needs to be at least");
    }

    #[test]
    fn test_min_size() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 4096]);
        ucmd.arg("-d")
            .arg("swap")
            .fails()
            .stderr_contains("swap space needs to be at least");
    }

    #[test]
    fn test_swapfile() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .succeeds()
            .code_is(0)
            .stdout_contains("Setting up swapspace version 1")
            .stdout_contains("insecure file owner");
    }

    #[test]
    fn test_swaplabel() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .arg("-l")
            .arg("SWAPLABEL")
            .succeeds()
            .code_is(0)
            .stdout_contains("LABEL=SWAPLABEL,")
            .stdout_contains("Setting up swapspace version 1");
    }

    #[test]
    fn test_custom_uuid() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .arg("-l")
            .arg("SWAP")
            .arg("-u")
            .arg("4adbb628-19fa-4bef-9c60-8ce030381672")
            .succeeds()
            .code_is(0)
            .stdout_contains("LABEL=SWAP, UUID=4adbb628-19fa-4bef-9c60-8ce030381672")
            .stdout_contains("Setting up swapspace version 1");
    }

    ///test truncation on a label that is above the 16 byte maximum
    #[test]
    fn test_long_label() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .arg("-l")
            .arg("OUTRAGEOUSLYLONGSWAPLABEL")
            .succeeds()
            .code_is(0)
            .stdout_contains("LABEL=OUTRAGEOUSLYLONG,")
            .stdout_contains("Setting up swapspace version 1");
    }

    #[test]
    fn test_check_blocks() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .arg("--check")
            .arg("--verbose")
            .succeeds()
            .code_is(0)
            .stdout_contains("Setting up swapspace version 1")
            .stdout_contains("0 bad pages");
    }

    #[test]
    fn test_invalid_uuid() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .arg("-l")
            .arg("SWAP")
            .arg("-u")
            .arg("078d9a95+4c1e-4961-b8a5-3f9d27586645")
            .fails()
            .code_is(2)
            .stderr_contains("Invalid UUID '078d9a95+4c1e-4961-b8a5-3f9d27586645':");
    }

    #[test]
    fn test_create_file() {
        use std::io::Read;
        let (at, mut ucmd) = at_and_ucmd!();
        ucmd.arg("-d")
            .arg("swapfile")
            .arg("-F")
            .arg("-s")
            .arg("65535")
            .succeeds()
            .code_is(0)
            .stdout_contains("Setting up swapspace version 1");
        at.file_exists("swapfile");

        let mut buf = vec![0u8; 4096];

        let mut fd = at.open("swapfile");
        fd.read_exact(&mut buf).unwrap();

        let sig = &buf[4086..];
        let swapsig = "SWAPSPACE2".as_bytes();
        assert_eq!(sig, swapsig);
    }
}
#[cfg(not(target_os = "linux"))]
mod non_linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .fails()
            .code_is(1)
            .stderr_is("mkswap: `mkswap` is available only on Linux.\n");
    }
}
