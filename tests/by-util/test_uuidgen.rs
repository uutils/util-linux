// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uuid::Uuid;
use uutests::{new_ucmd, util::UCommand};

fn assert_ver_eq(cmd: &mut UCommand, ver: uuid::Version) {
    let uuid = Uuid::parse_str(
        cmd.succeeds()
            .stdout_str()
            .strip_suffix('\n')
            .expect("newline"),
    )
    .expect("valid UUID");
    assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
    assert_eq!(uuid.get_version(), Some(ver));
}

#[test]
fn test_random() {
    assert_ver_eq(&mut new_ucmd!(), uuid::Version::Random);
    assert_ver_eq(new_ucmd!().arg("-r"), uuid::Version::Random);
    assert_ver_eq(new_ucmd!().arg("--random"), uuid::Version::Random);
}

#[test]
fn test_time() {
    assert_ver_eq(new_ucmd!().arg("-t"), uuid::Version::Mac);
    assert_ver_eq(new_ucmd!().arg("--time"), uuid::Version::Mac);
}

#[test]
fn test_arg_conflict() {
    new_ucmd!().args(&["-r", "-t"]).fails().code_is(1);
    new_ucmd!().args(&["--time", "--random"]).fails().code_is(1);
}

#[test]
fn test_md5_sha1() {
    new_ucmd!()
        .args(&["--namespace", "@dns", "--name", "example.com", "-m"])
        .succeeds()
        .stdout_only("9073926b-929f-31c2-abc9-fad77ae3e8eb\n");
    new_ucmd!()
        .args(&["-s", "--namespace", "@dns", "--name", "foobar"])
        .succeeds()
        .stdout_only("a050b517-6677-5119-9a77-2d26bbf30507\n");
    new_ucmd!()
        .args(&["-s", "--namespace", "@url", "--name", "foobar"])
        .succeeds()
        .stdout_only("8304efdd-bd6e-5b7c-a27f-83f3f05c64e0\n");
    new_ucmd!()
        .args(&["--sha1", "--namespace", "@oid", "--name", "foobar"])
        .succeeds()
        .stdout_only("364c03e1-bcdc-58bb-94ed-43e9a92f5f08\n");
    new_ucmd!()
        .args(&["--sha1", "--namespace", "@x500", "--name", "foobar"])
        .succeeds()
        .stdout_only("34da942e-f4a3-5169-9c65-267d2b22cf11\n");
}

#[test]
fn test_uuid_namespace() {
    // the @dns namespace written out, in both cases
    new_ucmd!()
        .args(&[
            "-m",
            "-n",
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "-N",
            "a",
        ])
        .succeeds()
        .stdout_only("4c104dd0-4821-30d5-9ce3-0e7a1f8b7c0d\n");
    new_ucmd!()
        .args(&[
            "-s",
            "-n",
            "6BA7B810-9DAD-11D1-80B4-00C04FD430C8",
            "-N",
            "a",
        ])
        .succeeds()
        .stdout_only("4f3f2898-69e3-5a0d-820a-c4e87987dbce\n");
    for namespace in [
        "{6ba7b810-9dad-11d1-80b4-00c04fd430c8}",
        "6ba7b8109dad11d180b400c04fd430c8",
        "urn:uuid:6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "6ba7b810-9dad-11d1-80b4-00c04fd430c",
    ] {
        new_ucmd!()
            .args(&["-s", "-n", namespace, "-N", "a"])
            .fails()
            .code_is(1)
            .stderr_contains(format!("Invalid namespace {namespace}."));
    }
}

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("-Z").fails().code_is(1);
    new_ucmd!().args(&["-r", "-Z"]).fails().code_is(1);
}

#[test]
fn test_name_namespace_on_non_hash() {
    new_ucmd!()
        .args(&["--namespace", "@dns", "-r"])
        .fails()
        .code_is(1);
    new_ucmd!()
        .args(&["--name", "example.com", "-r"])
        .fails()
        .code_is(1);
    new_ucmd!()
        .args(&["--namespace", "@dns", "--name", "example.com", "-r"])
        .fails()
        .code_is(1);
}

#[test]
fn test_missing_name_namespace() {
    new_ucmd!().arg("--sha1").fails().code_is(1);
    new_ucmd!()
        .args(&["--sha1", "--namespace", "@dns"])
        .fails()
        .code_is(1);
    new_ucmd!()
        .args(&["--sha1", "--name", "example.com"])
        .fails()
        .code_is(1);
}
