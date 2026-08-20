// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) axbxcx aZbxcx aZbZcZ axbxcZ aXbXcX
// spell-checker:ignore (words) rpmatch lnk dang

use uutests::{at_and_ucmd, new_ucmd};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

// -- the exit-status tally -----------------------------------------------

#[test]
fn test_a_plain_rename_reports_nothing_and_exits_zero() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["s", "z", "s1"]).succeeds().no_output();
    assert!(at.file_exists("z1"));
    assert!(!at.file_exists("s1"));
}

/// A no-match is not a failure and not a success: it is invisible to the tally.
#[test]
fn test_a_no_match_exits_four() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("n1");
    ucmd.args(&["q", "z", "n1"]).fails().code_is(4).no_output();
    assert!(at.file_exists("n1"));
}

// -- option permutation and POSIXLY_CORRECT -------------------------------

/// POSIXLY_CORRECT turns permutation off: scanning stops at the first operand
/// and everything after it is a filename, however much it looks like a flag.
#[test]
fn test_posixly_correct_stops_the_scan_at_the_first_operand() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.env("POSIXLY_CORRECT", "")
        .args(&["s", "z", "s1", "-v"])
        .fails()
        .code_is(2)
        .no_stdout();
    assert!(at.file_exists("z1"));
}

#[test]
fn test_posixly_correct_is_read_for_presence_and_not_for_value() {
    for value in ["", "0"] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        ucmd.env("POSIXLY_CORRECT", value)
            .args(&["s", "z", "s1", "-v"])
            .fails()
            .code_is(2)
            .no_stdout();
        assert!(at.file_exists("z1"), "{value:?}");
    }
}

/// Scanning stops at the first NON-OPTION, not at the first operand slot, so
/// flags written before it are still flags - and this invocation is short of
/// operands either way.
#[test]
fn test_posixly_correct_still_reads_the_flags_that_come_first() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.env("POSIXLY_CORRECT", "")
        .args(&["-v", "s", "z", "s1"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n");
    assert!(at.file_exists("z1"));
}

/// The existence check runs before the substitution result matters.
#[test]
fn test_a_missing_operand_fails_even_when_the_needle_cannot_match() {
    new_ucmd!()
        .args(&["q", "z", "s9"])
        .fails()
        .code_is(1)
        .no_stdout();
}

/// The truth table. No boundary between the statuses is documented, so this
/// walks every mix of the three per-operand outcomes: s1 s2 rename, s8 s9 are
/// absent, n1 n2 exist and hold no `s`. Two rows carry the rules a tally of
/// flags rather than counters would get wrong - S+N is 0 rather than 2 or 4,
/// and F+N is 1 rather than 2. The last three reverse a mix already listed,
/// because two counters cannot encode an order.
#[test]
fn test_the_tally_selects_one_status_for_every_mix_of_outcomes() {
    for (operands, code) in [
        (&["s1"][..], 0),
        (&["s9"], 1),
        (&["n1"], 4),
        (&["s1", "s2"], 0),
        (&["s1", "s9"], 2),
        (&["s1", "n1"], 0),
        (&["s8", "s9"], 1),
        (&["s9", "n1"], 1),
        (&["n1", "n2"], 4),
        (&["s1", "s9", "n1"], 2),
        (&["s9", "n1", "n2"], 1),
        (&["s9", "s1"], 2),
        (&["n1", "s1"], 0),
        (&["n1", "s9"], 1),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        for name in ["s1", "s2", "n1", "n2"] {
            at.touch(name);
        }
        let actual = ucmd.args(&["s", "z"]).args(operands).run().code();
        assert_eq!(actual, code, "operands {operands:?}");
    }
}

/// The other half of this claim, that each failure is reported exactly once
/// and in argv order, is pinned by
/// unix::test_every_failing_operand_is_reported_once_and_in_order, because the
/// text it has to assert is the platform's strerror.
#[test]
fn test_a_failure_in_the_middle_does_not_stop_the_operands_after_it() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s2");
    ucmd.args(&["-v", "s", "z", "s8", "s2", "s9"])
        .fails()
        .code_is(2)
        .stdout_is("`s2' -> `z2'\n");
    assert!(at.file_exists("z2"));
}

// -- the substring-equals-replacement short circuit ----------------------

/// The short circuit is for the whole run, before any operand is touched, so a
/// missing operand is not even noticed.
#[test]
fn test_an_identical_substring_and_replacement_touch_nothing() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["s", "s", "s1", "s9"])
        .fails()
        .code_is(4)
        .no_output();
    assert!(at.file_exists("s1"));
}

// -- reporting -----------------------------------------------------------

#[test]
fn test_verbose_names_both_paths() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["-v", "s", "z", "s1"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n");
}

/// The report delimits both names with a backtick and a quote and escapes
/// nothing, so a name containing either one goes out as it is.
#[test]
fn test_the_report_does_not_escape_its_own_delimiters() {
    for (name, needle, replacement, expected) in [
        ("q'te", "q", "Q", "`Q'te'"),
        ("sp ace", "sp", "SP", "`SP ace'"),
        ("t`ck", "t", "T", "`T`ck'"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch(name);
        ucmd.args(&["-v", needle, replacement, name])
            .succeeds()
            .stdout_is(format!("`{name}' -> {expected}\n"));
    }
}

/// -n counts as though the rename had happened and prints the same line.
#[test]
fn test_no_act_changes_nothing_but_still_reports() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["-nv", "s", "z", "s1"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n");
    assert!(at.file_exists("s1"));
    assert!(!at.file_exists("z1"));
}

// -- substitution modes --------------------------------------------------

/// Which occurrence each mode takes, and how each scan behaves where the
/// matches overlap. The overlap rows are the ones that separate the three
/// modes: -l scans backward rather than taking the last match a forward scan
/// would find, which is why `aaa` in `aaaa` gives `Za` one way and `aZ` the
/// other. The replacement is never rescanned, so replacing `a` by `aa`
/// terminates.
#[test]
fn test_the_modes_choose_which_occurrence_is_replaced() {
    for (flags, needle, replacement, name, new) in [
        ("-v", "x", "Z", "axbxcx", "aZbxcx"),
        ("-va", "x", "Z", "axbxcx", "aZbZcZ"),
        ("-vl", "x", "Z", "axbxcx", "axbxcZ"),
        ("-v", "aaa", "Z", "aaaa", "Za"),
        ("-va", "aaa", "Z", "aaaa", "Za"),
        ("-vl", "aaa", "Z", "aaaa", "aZ"),
        ("-va", "a", "aa", "aaaa", "aaaaaaaa"),
        ("-v", "x", "", "axbxcx", "abxcx"),
        ("-va", "x", "", "axbxcx", "abc"),
        ("-vl", "x", "", "axbxcx", "axbxc"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch(name);
        ucmd.args(&[flags, needle, replacement, name])
            .succeeds()
            .stdout_is(format!("`{name}' -> `{new}'\n"));
        assert!(at.file_exists(new), "{flags} {needle} {replacement} {name}");
    }
}

/// The count is one more than the number of code units in the name, which is
/// what makes the -a row a statement about the engine rather than about this
/// fixture.
#[test]
fn test_an_empty_needle_inserts_at_every_boundary() {
    for (flags, replacement, name, new) in [
        ("-v", "Z", "abc", "Zabc"),
        ("-vl", "Z", "abc", "abcZ"),
        ("-va", "Z", "abc", "ZaZbZcZ"),
        ("-va", "ZY", "abc", "ZYaZYbZYcZY"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch(name);
        ucmd.args(&[flags, "", replacement, name])
            .succeeds()
            .stdout_is(format!("`{name}' -> `{new}'\n"));
        assert!(at.file_exists(new), "{flags} '' {replacement} {name}");
    }
}

// -- path scope ----------------------------------------------------------

#[test]
fn test_only_the_final_component_is_rewritten() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    at.touch("d1/s1");
    ucmd.args(&["-v", "d1", "d9", "d1/s1"])
        .fails()
        .code_is(4)
        .no_output();
    assert!(at.file_exists("d1/s1"));
}

/// A separator in either argument widens the scope to the whole path, which is
/// how a rename moves a file between directories.
#[test]
fn test_a_separator_in_an_argument_moves_the_file() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    at.mkdir("d9");
    at.touch("d1/s1");
    ucmd.args(&["-v", "d1/", "d9/", "d1/s1"])
        .succeeds()
        .stdout_is("`d1/s1' -> `d9/s1'\n");
    assert!(at.file_exists("d9/s1"));
}

/// The scope is decided on the two argument strings, before any matching, so a
/// separator widens it even when the needle cannot possibly match.
#[test]
fn test_a_separator_in_either_argument_widens_the_scope() {
    // Only the substring holds a separator.
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    at.touch("d1/s1");
    ucmd.args(&["-v", "d1/s", "X", "d1/s1"])
        .succeeds()
        .stdout_is("`d1/s1' -> `X1'\n");
    assert!(at.file_exists("X1"));

    // Only the replacement does.
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    at.mkdir("d3");
    at.touch("d1/s1");
    ucmd.args(&["d1", "d3/", "d1/s1"]).succeeds();
    assert!(at.file_exists("d3/s1"));
}

/// The report shows the stripped form on both sides, however many separators
/// there were.
#[test]
fn test_trailing_separators_are_stripped_from_the_operand() {
    for (flags, needle, replacement, operand, expected) in [
        ("-v", "1", "9", "d1/", "`d1' -> `d9'\n"),
        ("-v", "1", "9", "d1///", "`d1' -> `d9'\n"),
        ("-v", "", "Y", "d1/", "`d1' -> `Yd1'\n"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("d1");
        ucmd.args(&[flags, needle, replacement, operand])
            .succeeds()
            .stdout_is(expected);
    }
}

/// Whole-path mode never splits a component out, so it has nothing to strip
/// and the trailing separator survives into the new name.
#[test]
fn test_whole_path_mode_keeps_a_trailing_separator() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    ucmd.args(&["-v", "d1/", "d9/", "d1/"])
        .succeeds()
        .stdout_is("`d1/' -> `d9/'\n");
    assert!(at.dir_exists("d9"));
}

/// The comparison that decides whether anything changed is between the
/// stripped name and the new one. Comparing against the raw operand instead
/// would find a difference here and report a rename of `d1` onto itself.
#[test]
fn test_the_change_comparison_happens_after_stripping() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    ucmd.args(&["-v", "QQ", "ZZ", "d1/"])
        .fails()
        .code_is(4)
        .no_output();
    assert!(at.dir_exists("d1"));
}

/// A leading `./` is prefix, not component, so it is neither matched against
/// nor lost.
#[test]
fn test_a_dot_relative_operand_keeps_its_prefix() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("top1");
    ucmd.args(&["-v", "top", "TOP", "./top1"])
        .succeeds()
        .stdout_is("`./top1' -> `./TOP1'\n");
    assert!(at.file_exists("TOP1"));

    // The dot lives in the prefix, so a needle of "." matches nothing.
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("top1");
    ucmd.args(&[".", "X", "./top1"]).fails().code_is(4);
    assert!(at.file_exists("top1"));
}

/// The stripping happens after the existence check, which still sees the
/// operand exactly as it was typed - so a trailing separator on a regular file
/// is an error rather than a rename. What the platform calls ENOTDIR is its
/// business, so only our half of the line is asserted.
#[test]
fn test_the_existence_check_sees_the_untouched_operand() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("d1");
    at.touch("d1/s1");
    ucmd.args(&["s", "z", "d1/s1/"])
        .fails()
        .code_is(1)
        .stderr_contains("rename: d1/s1/: not accessible: ");
    assert!(at.file_exists("d1/s1"));
}

// -- the overwrite safeguards --------------------------------------------

/// A quiet -o run says nothing at all where a quiet -i run still asks. With
/// -v the prompt and whatever follows share a line, because the prompt carries
/// no newline of its own.
#[test]
fn test_the_skip_report_is_verbose_gated_and_the_prompt_is_not() {
    for (flags, answer, code, expected) in [
        ("-o", "", 4, ""),
        ("-vo", "", 4, "Skipping existing file: `n1'\n"),
        ("-i", "n\n", 4, "rename: overwrite `n1'? "),
        (
            "-vi",
            "n\n",
            4,
            "rename: overwrite `n1'? Skipping existing file: `n1'\n",
        ),
        ("-i", "y\n", 0, "rename: overwrite `n1'? "),
        ("-vi", "y\n", 0, "rename: overwrite `n1'? `s1' -> `n1'\n"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("n1");
        let result = ucmd.args(&[flags, "s", "n", "s1"]).pipe_in(answer).run();
        assert_eq!(result.code(), code, "{flags} answered {answer:?}");
        result.no_stderr().stdout_is(expected);
    }
}

/// rpmatch under LC_ALL=C resolves to a test on the first character, and
/// uutests pins LC_ALL=C for every run. An implementation comparing the whole
/// answer against "y" would reject "yes", which is accepted; one that skipped
/// leading whitespace would accept " y", which is not.
#[test]
fn test_the_answer_is_matched_on_its_first_character() {
    for answer in ["y\n", "yes\n", "Y extra\n"] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("z1");
        ucmd.args(&["-i", "s", "z", "s1"])
            .pipe_in(answer)
            .succeeds()
            .stdout_is("rename: overwrite `z1'? ");
        assert!(
            !at.file_exists("s1"),
            "{answer:?} should have been accepted"
        );
    }

    for answer in ["n\n", "maybe\n", " y\n", "\n", "1\n"] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("z1");
        ucmd.args(&["-i", "s", "z", "s1"])
            .pipe_in(answer)
            .fails()
            .code_is(4)
            .stdout_is("rename: overwrite `z1'? ");
        assert!(at.file_exists("s1"), "{answer:?} should have been declined");
    }
}

/// Neither guard fires unless something is actually in the way, and -i does
/// not read stdin when it has nothing to ask about.
#[test]
fn test_an_absent_destination_is_never_guarded() {
    for flags in ["-vo", "-vi"] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        // The answer is never read, so the write end may break first. That it
        // breaks at all is the evidence, not an error.
        ucmd.args(&[flags, "s", "z", "s1"])
            .pipe_in("n\n")
            .ignore_stdin_write_error()
            .succeeds()
            .stdout_is("`s1' -> `z1'\n");
        assert!(at.file_exists("z1"));
    }
}

#[test]
fn test_no_overwrite_refuses_a_destination_of_any_kind() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    at.mkdir("z1");
    ucmd.args(&["-vo", "s", "z", "s1"])
        .fails()
        .code_is(4)
        .stdout_is("Skipping existing file: `z1'\n");
    assert!(at.file_exists("s1"));
}

/// An accepted prompt is a decision to try, not a promise that it works: the
/// rename can still fail, and then it is a failure like any other.
#[test]
fn test_an_accepted_prompt_that_cannot_be_carried_out_fails() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    at.mkdir("z1");
    at.touch("z1/keep");
    ucmd.args(&["-i", "s", "z", "s1"])
        .pipe_in("y\n")
        .fails()
        .code_is(1)
        .stdout_is("rename: overwrite `z1'? ");
    assert!(at.file_exists("s1"));
}

/// Under -n both guards still run and still report, and the verbose line
/// prints on top of the skip - two lines for an operand that counts as
/// neither, so the run reports 4 rather than 0. -i degrades to exactly what -o
/// does and never reads stdin, so no answer changes any of it.
#[test]
fn test_no_act_degrades_both_safeguards_to_the_same_skip() {
    for (flags, answer) in [("-nvo", ""), ("-nvi", "y\n"), ("-nvi", "n\n")] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("n1");
        // -n never reads stdin, so the write end may break before the answer
        // lands. That is the behavior under test, not a failure of it.
        ucmd.args(&[flags, "s", "n", "s1"])
            .pipe_in(answer)
            .ignore_stdin_write_error()
            .fails()
            .code_is(4)
            .stdout_is("Skipping existing file: `n1'\n`s1' -> `n1'\n");
        assert!(at.file_exists("s1"), "{flags} answered {answer:?}");
    }
}

/// A prompt that reaches end of input answers itself with a literal `n` and
/// terminates the line, so unlike every other decline this one does leave the
/// prompt's line closed. The echo is not verbose-gated, because the prompt it
/// closes is not either.
#[test]
fn test_interactive_at_end_of_input_declines() {
    for (flags, expected) in [
        (
            "-vi",
            "rename: overwrite `z1'? n\nSkipping existing file: `z1'\n",
        ),
        ("-i", "rename: overwrite `z1'? n\n"),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("z1");
        ucmd.args(&[flags, "s", "z", "s1"])
            .fails()
            .code_is(4)
            .stdout_is(expected);
        assert!(at.file_exists("s1"), "{flags}");
    }
}

/// A run that outlives its input reaches end of input on the second prompt
/// rather than re-reading the first answer, and a multi-character yes does not
/// spill into the answer the next prompt reads.
#[test]
fn test_each_prompt_consumes_a_whole_line() {
    for (answers, expected, second_renamed) in [
        (
            "y\n",
            "rename: overwrite `n1'? rename: overwrite `n2'? n\n",
            false,
        ),
        (
            "yn\n",
            "rename: overwrite `n1'? rename: overwrite `n2'? n\n",
            false,
        ),
        (
            "yes\ny\n",
            "rename: overwrite `n1'? rename: overwrite `n2'? ",
            true,
        ),
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        for name in ["s1", "s2", "n1", "n2"] {
            at.touch(name);
        }
        ucmd.args(&["-i", "s", "n", "s1", "s2"])
            .pipe_in(answers)
            .succeeds()
            .stdout_is(expected);
        assert!(!at.file_exists("s1"), "{answers:?}");
        assert_eq!(!at.file_exists("s2"), second_renamed, "{answers:?}");
    }
}

// -- the option surface --------------------------------------------------

/// Three operands are required and a usage error is exit 1, which is the same
/// status as "everything failed". The wording is clap's and is not asserted.
#[test]
fn test_too_few_operands_exit_one() {
    let cases: [&[&str]; 3] = [&[], &["s", "z"], &["-v", "s", "z"]];
    for args in cases {
        new_ucmd!().args(args).fails().code_is(1);
    }
}

/// The only two mutually exclusive pairs, in every spelling that reaches them.
/// Overriding an argument with itself does not weaken this: `-l -a -l` still
/// conflicts.
#[test]
fn test_the_exclusive_pairs_are_refused_however_they_are_spelled() {
    let cases: [&[&str]; 3] = [&["-a", "-l"], &["-o", "-i"], &["-l", "-a", "-l"]];
    for args in cases {
        new_ucmd!()
            .args(args)
            .args(&["x", "Z", "axbxcx"])
            .fails()
            .code_is(1);
    }
}

/// Every option has a long form and clap is configured to accept any
/// unambiguous prefix of one. A prefix shared by two options is an error
/// rather than a choice: `--n` reaches both of the no- options.
#[test]
fn test_a_long_option_may_be_abbreviated_to_an_unambiguous_prefix() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["--verb", "s", "z", "s1"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n");

    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["--n", "s", "z", "s1"]).fails().code_is(1);
    assert!(at.file_exists("s1"));
}

/// Options are permuted: a flag is recognized anywhere, including between the
/// two arguments and between two file operands.
#[test]
fn test_a_flag_is_recognized_anywhere_among_the_operands() {
    for args in [
        &["-v", "s", "z", "s1"][..],
        &["s", "-v", "z", "s1"],
        &["s", "z", "-v", "s1"],
        &["s", "z", "s1", "-v"],
    ] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        ucmd.args(args).succeeds().stdout_is("`s1' -> `z1'\n");
    }

    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    at.touch("s2");
    ucmd.args(&["s", "z", "s1", "-v", "s2"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n`s2' -> `z2'\n");
}

#[test]
fn test_a_terminator_turns_everything_after_it_into_an_operand() {
    for args in [&["--", "s", "z", "s1"][..], &["s", "z", "--", "s1"]] {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        ucmd.args(args).succeeds();
        assert!(at.file_exists("z1"), "{args:?}");
    }

    // The flag is an operand now, and a missing one at that.
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["--", "s", "z", "s1", "-v"])
        .fails()
        .code_is(2)
        .no_stdout();
    assert!(at.file_exists("z1"));
}

/// getopt lets a flag repeat and C util-linux renames regardless, so this is
/// behavior rather than wording: a clap SetTrue argument rejects its second
/// occurrence unless the command overrides an argument with itself.
#[test]
fn test_a_repeated_option_is_accepted() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("s1");
    ucmd.args(&["-v", "-v", "s", "z", "s1"])
        .succeeds()
        .stdout_is("`s1' -> `z1'\n");

    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("axbxcx");
    ucmd.args(&["-a", "-a", "x", "Z", "axbxcx"]).succeeds();
    assert!(at.file_exists("aZbZcZ"));
}

/// Symlink mode, byte-oriented names, mode bits and POSIX rename(2) semantics
/// all need helpers that do not exist or do not behave the same on Windows, so
/// they are gathered here rather than gated one attribute at a time. Everything
/// above this line runs on all three platforms.
#[cfg(unix)]
mod unix {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use uutests::util::AtPath;
    use uutests::{at_and_ucmd, new_ucmd};

    /// Tests that arrange for a mode bit to deny something expect that denial
    /// to happen. Root is not denied by any mode bit, so they are skipped
    /// there rather than made to pass by accident. CI never runs as root; this
    /// is for the developer who does.
    fn skipped_as_root() -> bool {
        if uucore::process::geteuid() == 0 {
            println!("test skipped: root is not denied by any mode bit");
            return true;
        }
        false
    }

    // -- error classes ----------------------------------------------------

    /// The diagnostic carries the platform's strerror text, which is why this
    /// half lives here and its other half,
    /// test_a_failure_in_the_middle_does_not_stop_the_operands_after_it,
    /// stays ungated.
    #[test]
    fn test_every_failing_operand_is_reported_once_and_in_order() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s2");
        ucmd.args(&["-v", "s", "z", "s8", "s2", "s9"])
            .fails()
            .code_is(2)
            .stdout_is("`s2' -> `z2'\n")
            .stderr_is(
                "rename: s8: not accessible: No such file or directory\n\
                 rename: s9: not accessible: No such file or directory\n",
            );
        assert!(at.file_exists("z2"));
    }

    /// A fixture builder, the argv that fails against it, and the diagnostic
    /// the failure produces.
    type ErrorCase<'a> = (fn(&AtPath), Vec<&'a str>, String);

    /// The tally counts failures, not kinds of failure. Each of these is a
    /// different errno reached through a different call, and every one of them
    /// is exactly one failure and nothing else. Only the part of the
    /// diagnostic we write is asserted - the errno text belongs to libc.
    #[test]
    fn test_every_error_class_counts_as_one_failure() {
        let cases: [ErrorCase; 2] = [
            (
                |at| {
                    at.touch("top1");
                    at.mkdir("d3");
                    at.touch("d3/keep");
                },
                vec!["top1", "d3", "top1"],
                "rename: top1: rename to d3 failed: ".into(),
            ),
            (
                |at| {
                    at.mkdir("d1");
                    at.mkdir("d3");
                    at.touch("d3/keep");
                },
                vec!["d1", "d3", "d1"],
                "rename: d1: rename to d3 failed: ".into(),
            ),
        ];

        for (setup, args, expected) in cases {
            let (at, mut ucmd) = at_and_ucmd!();
            setup(&at);
            ucmd.args(&args)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains(&expected);
        }
    }

    /// A directory that denies writes fails the rename; one that denies search
    /// fails the existence check instead, and the two say different things.
    /// Which they are is the claim; the errno text after them is the
    /// platform's and is left to it.
    #[test]
    fn test_a_denied_directory_fails_at_whichever_step_needs_it() {
        if skipped_as_root() {
            return;
        }

        for (mode, expected) in [
            (0o500, "rename: p/s1: rename to p/z1 failed: "),
            (0o000, "rename: p/s1: not accessible: "),
        ] {
            let (at, mut ucmd) = at_and_ucmd!();
            at.mkdir("p");
            at.touch("p/s1");
            at.set_mode("p", mode);
            ucmd.args(&["s", "z", "p/s1"])
                .fails()
                .code_is(1)
                .stderr_contains(expected);
            at.set_mode("p", 0o700);
            assert!(at.file_exists("p/s1"));
        }
    }

    /// A replacement that deletes the whole name is not caught before the
    /// syscall: the empty string is handed to rename(2) and the kernel refuses
    /// it.
    #[test]
    fn test_a_replacement_that_empties_the_name_fails_in_the_kernel() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("abc");
        ucmd.args(&["-v", "abc", "", "abc"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("rename: abc: rename to  failed: No such file or directory\n");
        assert!(at.file_exists("abc"));
    }

    // -- byte-oriented names ----------------------------------------------

    /// Nothing quotes or escapes a name, in the report or in a diagnostic.
    #[test]
    fn test_a_name_holding_a_newline_splits_the_line_it_is_printed_on() {
        let (at, mut ucmd) = at_and_ucmd!();
        let name = OsStr::from_bytes(b"nl\nx");
        at.touch(name);
        ucmd.args(&[OsStr::new("-v"), OsStr::new("nl"), OsStr::new("NL"), name])
            .succeeds()
            .stdout_is("`nl\nx' -> `NL\nx'\n");
        assert!(at.file_exists(OsStr::from_bytes(b"NL\nx")));

        new_ucmd!()
            .args(&[
                OsStr::new("-v"),
                OsStr::new("no"),
                OsStr::new("yes"),
                OsStr::from_bytes(b"no\nsuch"),
            ])
            .fails()
            .code_is(1)
            .stderr_is("rename: no\nsuch: not accessible: No such file or directory\n");
    }

    // -- POSIX rename(2) semantics ----------------------------------------

    /// A substitution that only rewrites a separator names the very same file,
    /// and that still counts as a rename.
    #[test]
    fn test_a_rename_that_only_changes_the_string_still_counts() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("d1");
        at.touch("d1/s1");
        ucmd.args(&["-v", "/", "//", "d1/s1"])
            .succeeds()
            .stdout_is("`d1/s1' -> `d1//s1'\n");
        assert!(at.file_exists("d1/s1"));
    }

    // -- the overwrite safeguards, where they need a link -----------------

    /// -o asks whether anything is reachable at the destination, which follows
    /// the link: a dangling symlink is not something, so it gets clobbered.
    #[test]
    fn test_no_overwrite_does_not_protect_a_dangling_destination() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.relative_symlink_file("t_missing", "n1");
        ucmd.args(&["-vo", "s", "n", "s1"])
            .succeeds()
            .stdout_is("`s1' -> `n1'\n");
        assert!(!at.symlink_exists("n1"));
        assert!(at.file_exists("n1"));
    }

    /// A probe that cannot be performed at all reads as "nothing there", so -o
    /// lets the rename go ahead and it fails on its own terms. Both runs below
    /// report the same thing, which is the whole point: -o changes nothing
    /// when it cannot see the destination.
    #[test]
    fn test_no_overwrite_treats_a_denied_probe_as_absent() {
        if skipped_as_root() {
            return;
        }

        for flags in [&["-v", "-o"][..], &["-v"]] {
            let (at, mut ucmd) = at_and_ucmd!();
            at.mkdir("d1");
            at.touch("d1/s1");
            at.mkdir("e");
            at.touch("e/s1");
            at.set_mode("e", 0o000);
            ucmd.args(flags)
                .args(&["d1/", "e/", "d1/s1"])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains("rename: d1/s1: rename to e/s1 failed: ");
            at.set_mode("e", 0o700);
            assert!(at.file_exists("d1/s1"));
        }
    }

    // -- symlink mode -----------------------------------------------------

    #[test]
    fn test_symlink_mode_rewrites_the_target_not_the_name() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("t1");
        at.relative_symlink_file("t1", "l_ok");
        ucmd.args(&["-vs", "t", "z", "l_ok"])
            .succeeds()
            .stdout_is("l_ok: `t1' -> `z1'\n");
        assert_eq!(at.resolve_link("l_ok"), "z1");
    }

    /// The symlink type check precedes the match test, so a non-symlink fails
    /// even when the substring appears nowhere.
    #[test]
    fn test_symlink_mode_on_a_regular_file_fails_without_matching() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("t1");
        ucmd.args(&["-s", "q", "Q", "t1"])
            .fails()
            .code_is(1)
            .stderr_is("rename: t1: not a symbolic link\n");
    }

    /// -s never resolves a chain: the target text of l_c1 is "l_c2", not "t1".
    #[test]
    fn test_symlink_mode_does_not_follow_a_chain() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("t1");
        at.relative_symlink_file("t1", "l_c2");
        at.relative_symlink_file("l_c2", "l_c1");
        ucmd.args(&["-s", "t", "z", "l_c1"]).fails().code_is(4);
        assert_eq!(at.resolve_link("l_c1"), "l_c2");
    }

    /// One tree of links covering every target shape -s has to handle.
    fn create_links(at: &AtPath) {
        at.touch("t1");
        at.mkdir("sub");
        at.touch("sub/t2");
        at.relative_symlink_file("t1", "l_ok");
        at.relative_symlink_file("t_missing", "l_dangle");
        at.relative_symlink_file("sub", "l_dir");
        at.relative_symlink_file("sub/t2", "l_deep");
        at.relative_symlink_file("/nonexistent/t1", "l_abs");
        at.relative_symlink_file("axbxcx", "l_rep");
        at.relative_symlink_file("t1", "l_c2");
        at.relative_symlink_file("l_c2", "l_c1");
        at.relative_symlink_file("l_self", "l_self");
        at.relative_symlink_file("t2", "sub/l_x");
    }

    /// -s rewrites the target text and nothing else, whatever that text is: a
    /// target that leads nowhere, an absolute one, an empty needle inserting
    /// into one, and a mode flag choosing among several matches in one.
    #[test]
    fn test_symlink_mode_rewrites_a_target_of_any_shape() {
        let cases: [(&[&str], &str, &str); 4] = [
            (&["-s", "t", "z", "l_dangle"], "l_dangle", "z_missing"),
            (&["-s", "t", "z", "l_abs"], "l_abs", "/nonexistent/z1"),
            (&["-s", "", "P", "l_deep"], "l_deep", "sub/Pt2"),
            (&["-s", "-a", "x", "X", "l_rep"], "l_rep", "aXbXcX"),
        ];

        for (args, link, target) in cases {
            let (at, mut ucmd) = at_and_ucmd!();
            create_links(&at);
            ucmd.args(args).succeeds();
            assert_eq!(at.resolve_link(link), target, "{args:?}");
        }
    }

    /// The target is scoped exactly the way a filename is: the final component
    /// only, unless an argument holds a separator. A needle that matches only
    /// the directory part matches nothing, and so does one that matches the
    /// link's own name rather than its target.
    #[test]
    fn test_symlink_mode_scopes_the_target_like_a_filename() {
        let cases: [(&[&str], &str, &str); 2] = [
            (&["-s", "sub", "SUB", "l_deep"], "l_deep", "sub/t2"),
            (&["-s", "ok", "OK", "l_ok"], "l_ok", "t1"),
        ];

        for (args, link, target) in cases {
            let (at, mut ucmd) = at_and_ucmd!();
            create_links(&at);
            ucmd.args(args).fails().code_is(4).no_output();
            assert_eq!(at.resolve_link(link), target, "{args:?}");
        }
    }

    /// The type error is an ordinary per-operand failure, so it mixes with a
    /// success the way any other failure does.
    #[test]
    fn test_symlink_mode_mixes_a_type_error_into_the_tally() {
        let (at, mut ucmd) = at_and_ucmd!();
        create_links(&at);
        ucmd.args(&["-s", "-v", "t", "z", "t1", "l_ok"])
            .fails()
            .code_is(2)
            .stdout_is("l_ok: `t1' -> `z1'\n")
            .stderr_is("rename: t1: not a symbolic link\n");
        assert_eq!(at.resolve_link("l_ok"), "z1");
    }

    /// Without -s a symlink is an ordinary directory entry: its name is
    /// rewritten and its target is left alone.
    ///
    /// This rename differs only in case, so neither a lookup of the old name
    /// nor one of the new name can tell whether anything happened on a
    /// filesystem that folds case - both would answer about the same entry.
    /// The stored spelling has to come from the directory itself, which is
    /// also the stronger assertion here: it rejects a rename to any name other
    /// than exactly `l_OK`.
    #[test]
    fn test_a_link_without_symlink_mode_is_renamed_by_name() {
        let (at, mut ucmd) = at_and_ucmd!();
        create_links(&at);
        ucmd.args(&["_ok", "_OK", "l_ok"]).succeeds();
        assert_eq!(at.resolve_link("l_OK"), "t1");

        let names: Vec<_> = std::fs::read_dir(at.plus("."))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect();
        // The positive assertion guards the negative one: a listing that could
        // not be read is empty, and would otherwise report the old name gone.
        assert!(names.iter().any(|name| name == "l_OK"), "{names:?}");
        assert!(!names.iter().any(|name| name == "l_ok"), "{names:?}");
    }

    /// Under -s the guard asks whether the new target NAME is a directory
    /// entry, not whether the link would resolve. `sub` is taken and `zzz` is
    /// not.
    #[test]
    fn test_symlink_mode_no_overwrite_probes_the_new_target_name() {
        let cases: [(&[&str], &str, &str, i32); 2] = [
            (&["-s", "-o", "t1", "sub", "l_ok"], "l_ok", "t1", 4),
            (&["-s", "-o", "t1", "zzz", "l_ok"], "l_ok", "zzz", 0),
        ];

        for (args, link, target, code) in cases {
            let (at, mut ucmd) = at_and_ucmd!();
            create_links(&at);
            let actual = ucmd.args(args).run().code();
            assert_eq!(actual, code, "{args:?}");
            assert_eq!(at.resolve_link(link), target, "{args:?}");
        }

        // The skip line names the link and the target it still has, not the
        // one it was refused.
        let (at, mut ucmd) = at_and_ucmd!();
        create_links(&at);
        ucmd.args(&["-s", "-o", "-v", "t1", "sub", "l_ok"])
            .fails()
            .code_is(4)
            .stdout_is("Skipping existing link: `l_ok' -> `t1'\n");
    }

    /// Under -s the prompt names the new target rather than the link, which is
    /// the opposite way round from the skip line just above.
    #[test]
    fn test_symlink_mode_prompts_about_the_new_target() {
        let cases: [(&[&str], &str, i32, &str, &str); 2] = [
            (
                &["-s", "-i", "t1", "sub", "l_ok"],
                "y\n",
                0,
                "rename: overwrite `sub'? ",
                "sub",
            ),
            (
                &["-s", "-v", "-i", "t1", "sub", "l_ok"],
                "n\n",
                4,
                "rename: overwrite `sub'? Skipping existing link: `l_ok' -> `t1'\n",
                "t1",
            ),
        ];

        for (args, answer, code, expected, target) in cases {
            let (at, mut ucmd) = at_and_ucmd!();
            create_links(&at);
            let result = ucmd.args(args).pipe_in(answer).run();
            assert_eq!(result.code(), code, "{args:?}");
            result.stdout_is(expected);
            assert_eq!(at.resolve_link("l_ok"), target, "{args:?}");
        }
    }

    /// The rewrite is not atomic and deliberately so: the unlink has already
    /// happened when the symlink fails, and the original link is gone. The
    /// message names the target it tried to create, so an empty target prints
    /// two spaces.
    ///
    /// Only Linux can be made to fail this way: its symlink(2) returns ENOENT
    /// when the target is an empty string, where the BSD call that macOS
    /// inherits reserves that error for an empty link name and creates the
    /// empty target happily.
    #[cfg(target_os = "linux")]
    #[test]
    fn test_a_symlink_that_cannot_be_created_leaves_no_link_behind() {
        let (at, mut ucmd) = at_and_ucmd!();
        create_links(&at);
        ucmd.args(&["-s", "t1", "", "l_ok"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("rename: l_ok: symlinking to  failed: No such file or directory\n");
        assert!(!at.symlink_exists("l_ok"));
    }

    /// Under -s the -o guard is an lstat on the new target, so a dangling entry
    /// there blocks the rewrite, where the default path stats instead, finds
    /// nothing, and lets the rename clobber it. The two modes need different
    /// predicates and must not share one helper.
    #[test]
    fn test_symlink_mode_no_overwrite_is_blocked_by_a_dangling_new_target() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.relative_symlink_file("gone", "old_t");
        at.relative_symlink_file("also_gone", "new_t");
        at.relative_symlink_file("old_t", "lnk");
        ucmd.args(&["-vos", "old_t", "new_t", "lnk"])
            .fails()
            .code_is(4)
            .stdout_is("Skipping existing link: `lnk' -> `old_t'\n");
        assert_eq!(at.resolve_link("lnk"), "old_t");
    }

    /// The rewrite is an unlink followed by a symlink, and the two halves
    /// report separately: a link whose parent directory denies writes fails at
    /// the unlink, keeps the link, and says which step it was.
    #[test]
    fn test_symlink_mode_reports_a_failed_unlink() {
        if skipped_as_root() {
            return;
        }

        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("p");
        at.touch("t1");
        at.relative_symlink_file("t1", "p/l_ro");
        at.set_mode("p", 0o500);
        ucmd.args(&["-s", "t", "z", "p/l_ro"])
            .fails()
            .code_is(1)
            .stderr_contains("rename: p/l_ro: unlink failed: ");
        at.set_mode("p", 0o700);
        assert_eq!(at.resolve_link("p/l_ro"), "t1");
    }

    /// The -s guard resolves a relative new target against the process working
    /// directory rather than the link's own directory: `sub/b1` exists, but the
    /// probe is for `b1`, which does not, so -o does not block the rewrite.
    #[test]
    fn test_symlink_mode_no_overwrite_probes_relative_to_the_working_directory() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("sub");
        at.touch("sub/a1");
        at.touch("sub/b1");
        at.relative_symlink_file("a1", "sub/lnk");
        ucmd.args(&["-vos", "a", "b", "sub/lnk"])
            .succeeds()
            .stdout_is("sub/lnk: `a1' -> `b1'\n");
        assert_eq!(at.resolve_link("sub/lnk"), "b1");
    }
}

/// Two things need more than a unix: a stdout that refuses everything written
/// to it needs /dev/full, which is a Linux device, and a filename that is not
/// valid UTF-8 needs a filesystem that stores names as bytes. APFS, the
/// default on macOS, accepts only valid UTF-8 for creation, so making one
/// there fails with EILSEQ before the utility is ever invoked.
#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsStr;
    use std::fs::{File, OpenOptions};
    use std::os::unix::ffi::OsStrExt;
    use uutests::{at_and_ucmd, new_ucmd};

    fn dev_full() -> Option<File> {
        match OpenOptions::new().write(true).open("/dev/full") {
            Ok(file) => Some(file),
            Err(_) => {
                println!("test skipped: /dev/full is not available");
                None
            }
        }
    }

    // -- byte-oriented names ----------------------------------------------

    /// A filename is a byte string, and one that is not valid UTF-8 goes
    /// through unchanged - matched as bytes, written to stdout as bytes, and
    /// created as bytes.
    #[test]
    fn test_a_name_that_is_not_valid_utf8_survives_the_round_trip() {
        let (at, mut ucmd) = at_and_ucmd!();
        let old = OsStr::from_bytes(b"lat\xe9n");
        at.touch(old);
        ucmd.args(&[OsStr::new("-v"), OsStr::new("lat"), OsStr::new("LAT"), old])
            .succeeds()
            .stdout_is_bytes(b"`lat\xe9n' -> `LAT\xe9n'\n");
        assert!(at.file_exists(OsStr::from_bytes(b"LAT\xe9n")));
    }

    /// The result is not valid UTF-8 and that is not an error. An engine
    /// working over characters could not express any of these.
    #[test]
    fn test_a_needle_may_split_a_multibyte_character() {
        for (needle, replacement, new) in [
            (&b"\xc3"[..], &b"X"[..], &b"cafX\xa9"[..]),
            (&b"f\xc3"[..], &b"F"[..], &b"caF\xa9"[..]),
        ] {
            let (at, mut ucmd) = at_and_ucmd!();
            let name = OsStr::from_bytes(b"caf\xc3\xa9");
            at.touch(name);
            ucmd.args(&[
                OsStr::from_bytes(needle),
                OsStr::from_bytes(replacement),
                name,
            ])
            .succeeds();
            assert!(at.file_exists(OsStr::from_bytes(new)), "{needle:?}");
        }
    }

    /// The unit is a byte: `caf<c3><a9>` is five bytes and takes six
    /// insertions, where an engine seeing four characters would insert five
    /// times - which is the whole reason the engine is generic over the code
    /// unit.
    #[test]
    fn test_an_empty_needle_counts_code_units_not_characters() {
        let (at, mut ucmd) = at_and_ucmd!();
        let name = OsStr::from_bytes(b"caf\xc3\xa9");
        at.touch(name);
        ucmd.args(&[OsStr::new("-a"), OsStr::new(""), OsStr::new("_"), name])
            .succeeds();
        assert!(at.file_exists(OsStr::from_bytes(b"_c_a_f_\xc3_\xa9_")));
    }

    // -- an unwritable stdout ---------------------------------------------

    /// Reporting is not best-effort. A run whose output cannot be written says
    /// so once, at the end, and reports 1 - discarding a tally that had
    /// already decided otherwise, even though every rename really happened.
    /// A run that writes nothing never notices.
    #[test]
    fn test_a_stdout_that_cannot_be_written_overrides_the_tally() {
        let cases: [(&[&str], i32, &str); 2] = [
            (
                &["-v", "s", "z", "s1", "s2"],
                1,
                "rename: write error: No space left on device\n",
            ),
            (&["s", "z", "s1", "s2"], 0, ""),
        ];

        for (args, code, expected) in cases {
            let Some(sink) = dev_full() else { return };

            let (at, mut ucmd) = at_and_ucmd!();
            at.touch("s1");
            at.touch("s2");
            let result = ucmd.args(args).set_stdout(sink).run();
            assert_eq!(result.code(), code, "{args:?}");
            result.stderr_is(expected);
        }

        // The renames the failing run reported are still renames.
        let Some(sink) = dev_full() else { return };
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        at.touch("s2");
        ucmd.args(&["-v", "s", "z", "s1", "s2"])
            .set_stdout(sink)
            .fails()
            .code_is(1);
        assert!(at.file_exists("z1"));
        assert!(at.file_exists("z2"));
    }

    /// The override is not a property of stdout. A diagnostic that cannot be
    /// written discards the tally the same way - and, more importantly, does
    /// not stop the run: the operand after the failing one is still renamed
    /// and its report still reaches a stdout that works.
    #[test]
    fn test_a_stderr_that_cannot_be_written_overrides_the_tally_and_does_not_stop_the_run() {
        let Some(sink) = dev_full() else { return };
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("s1");
        let result = ucmd
            .args(&["-v", "s", "z", "s9", "s1"])
            .set_stderr(sink)
            .run();
        assert_eq!(result.code(), 1);
        result.stdout_is("`s1' -> `z1'\n");
        assert!(at.file_exists("z1"));
    }

    /// Help is written by clap rather than by us, but a failed write is still a
    /// failed write: it is reported and it is worth 1, not a panic.
    #[test]
    fn test_a_clap_stream_that_cannot_be_written_is_reported() {
        for args in [&["--help"], &["--version"]] {
            let Some(sink) = dev_full() else { return };
            let mut ucmd = new_ucmd!();
            let result = ucmd.args(args).set_stdout(sink).run();
            assert_eq!(result.code(), 1, "{args:?}");
            result.stderr_is("rename: write error: No space left on device\n");
        }
    }
}
