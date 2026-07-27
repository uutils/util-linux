// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;
use std::ffi::OsString;
use std::process::{Command, Output};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_invalid_file() {
    new_ucmd!().arg("not_existing_file.not_existing_extension").fails().code_is(1);
}

// wall does not print the content of the file in the stdout, it sends it to the tty(s)
// Hence the use of cat to check if the get_message function can extract correctly the file

// #[test]
// fn test_retreive_message_from_a_file() {
//     let file = String::from("Cargo.toml");
//     let mut command = Command::new("cat");
//     command.arg(&file);
//     let output: Output = command.output().expect("Failed to start 'cat' command");
//     assert!(
//         output.status.success(),
//         "'cat' command exit with failure status"
//     );
//     let command_output =
//         String::from_utf8(output.stdout).expect("Failed to convert 'cat'output");

//     let command = vec!["wall", &file];
//     let matches = uucore::clap_localization::handle_clap_result(uu_app(), command)
//         .expect("External error");
//     let pos_arg = matches.get_many(STRING).unwrap_or_default();
//     let function_output = get_message(pos_arg).unwrap();
//     assert_eq!(function_output, command_output);
// }

// #[test]
// fn test_get_message_on_stdin() {
//     // Requires input by users
//     let command = vec!["wall"];
//     let matches = uucore::clap_localization::handle_clap_result(uu_app(), command)
//         .expect("External error");
//     let pos_arg = matches.get_many(STRING).unwrap_or_default();
//     let function_output = get_message(pos_arg).unwrap();
//     assert_eq!(function_output, "Hello !\n");
// }

// #[test]
// fn test_arguments_as_message() {
//     let command = vec!["wall", "Hello", "World", "!"];
//     let matches = uucore::clap_localization::handle_clap_result(uu_app(), command)
//         .expect("External error");
//     let pos_arg = matches.get_many(STRING).unwrap_or_default();
//     let function_output = get_message(pos_arg).unwrap();
//     assert_eq!(function_output, "Hello World !");
// }

// #[test]
// fn test_found_connected_users() {
//     let users = find_logged_users();
//     assert_eq!(
//         users,
//         vec!(
//             OsString::from("/dev/tty2"),
//             OsString::from("/dev/pts/1"),
//             OsString::from("/dev/pts/2")
//         )
//     );
// }

// #[test]
// fn test_print_to_terminals() {
//     let users = find_logged_users();
//     let _ = write_to_terminals(String::from("hello world!"), users);
//     let _ = write_to_terminals(
//         String::from("hello world!"),
//         vec![OsString::from("/dev/tty1")],
//     );
// }

// #[test]
// fn test_get_sender() {
//     let sender = crate::get_sender();
//     assert_eq!(sender, "pts/0");
// }
