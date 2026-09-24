// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (words) axbxcx abxcx aaaaaaaa abab abcZ axbxc axbxcZ
// spell-checker:ignore (words) aZbxcx aZbZcZ Zabc ZaZbZcZ ZxZ ZYaZYbZYcZY

//! The substitution engine. Pure: no filesystem, no syscalls, no encoding
//! assumptions. Generic over the filename code unit so the same rules apply to
//! bytes on unix and to UTF-16 units on Windows.

/// Which occurrence of the substring a run replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    First,
    All,
    Last,
}

/// Replace occurrences of `needle` in `name` with `replacement`.
///
/// Returns the new name, which equals `name` when nothing matched.
pub(crate) fn substitute<T: Copy + PartialEq>(
    name: &[T],
    needle: &[T],
    replacement: &[T],
    mode: Mode,
) -> Vec<T> {
    if needle.is_empty() {
        return interleave(name, replacement, mode);
    }

    match mode {
        Mode::First => match find(name, needle) {
            Some(at) => splice(name, at, needle.len(), replacement),
            None => name.to_vec(),
        },
        Mode::Last => match rfind(name, needle) {
            Some(at) => splice(name, at, needle.len(), replacement),
            None => name.to_vec(),
        },
        Mode::All => {
            let mut out = Vec::with_capacity(name.len());
            let mut rest = name;
            while let Some(at) = find(rest, needle) {
                out.extend_from_slice(&rest[..at]);
                out.extend_from_slice(replacement);
                rest = &rest[at + needle.len()..];
            }
            out.extend_from_slice(rest);
            out
        }
    }
}

/// One name before and after the substitution.
///
/// `old` is not always the string that was passed in: outside whole-path mode
/// the trailing separators are stripped off, and both the messages and the
/// rename itself use the stripped form.
pub(crate) struct Rewrite<'a, T> {
    pub(crate) old: &'a [T],
    pub(crate) new: Vec<T>,
}

impl<T: PartialEq> Rewrite<'_, T> {
    /// Nothing to do. C util-linux cannot tell this apart from a name that
    /// never matched, and counts both as neither renamed nor failed.
    pub(crate) fn is_unchanged(&self) -> bool {
        self.old == self.new
    }
}

/// Apply the substitution at the right scope.
///
/// Normally only the final path component changes. If either argv string
/// contains a separator the whole path is in scope, which is what lets a rename
/// move a file between directories. The rule is decided on the two strings
/// themselves, so it holds even when the needle cannot match anything.
///
/// Symlink mode feeds the link's target text through here unchanged: the target
/// is scoped the same way, so `-s sub SUB` leaves a target of `sub/t2` alone.
pub(crate) fn rewrite<'a, T: Copy + PartialEq>(
    name: &'a [T],
    needle: &[T],
    replacement: &[T],
    mode: Mode,
    sep: T,
) -> Rewrite<'a, T> {
    if needle.contains(&sep) || replacement.contains(&sep) {
        return Rewrite {
            old: name,
            new: substitute(name, needle, replacement, mode),
        };
    }

    let (prefix, component) = split_component(name, sep);

    let mut new = Vec::with_capacity(name.len());
    new.extend_from_slice(prefix);
    new.extend_from_slice(&substitute(component, needle, replacement, mode));

    Rewrite {
        old: &name[..prefix.len() + component.len()],
        new,
    }
}

/// Split a name into everything before its final component and the component
/// itself. Trailing separators belong to neither and are dropped, which is what
/// the -v line shows: `d1/` is reported as `d1`.
///
/// A name that is nothing but separators is the exception, and it is why the
/// second search stops one unit short: there is nothing to strip without
/// emptying the name, so the component becomes the final separator alone and
/// everything before it is the prefix. For every other name the unit at
/// `end - 1` is a non-separator by construction, so the shorter search finds
/// the same separator the full one would.
fn split_component<T: Copy + PartialEq>(name: &[T], sep: T) -> (&[T], &[T]) {
    let end = name
        .iter()
        .rposition(|unit| *unit != sep)
        .map_or(name.len(), |last| last + 1);
    let start = name[..end.saturating_sub(1)]
        .iter()
        .rposition(|unit| *unit == sep)
        .map_or(0, |at| at + 1);
    (&name[..start], &name[start..end])
}

/// An empty needle matches between every two code units. `First` prepends,
/// `Last` appends, and `All` inserts at every boundary including both ends -
/// n + 1 times for a name of n code units.
fn interleave<T: Copy>(name: &[T], replacement: &[T], mode: Mode) -> Vec<T> {
    match mode {
        Mode::First => [replacement, name].concat(),
        Mode::Last => [name, replacement].concat(),
        Mode::All => {
            let mut out = Vec::with_capacity(replacement.len() * (name.len() + 1) + name.len());
            for unit in name {
                out.extend_from_slice(replacement);
                out.push(*unit);
            }
            out.extend_from_slice(replacement);
            out
        }
    }
}

/// The index of the first occurrence of `needle`, which must not be empty.
fn find<T: Copy + PartialEq>(name: &[T], needle: &[T]) -> Option<usize> {
    name.windows(needle.len()).position(|w| w == needle)
}

/// The index of the last occurrence of `needle`, which must not be empty.
fn rfind<T: Copy + PartialEq>(name: &[T], needle: &[T]) -> Option<usize> {
    name.windows(needle.len()).rposition(|w| w == needle)
}

fn splice<T: Copy>(name: &[T], at: usize, len: usize, replacement: &[T]) -> Vec<T> {
    let mut out = Vec::with_capacity(name.len() + replacement.len());
    out.extend_from_slice(&name[..at]);
    out.extend_from_slice(replacement);
    out.extend_from_slice(&name[at + len..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rewritten name, for the many cases where the old side is not the
    /// point of the test.
    fn rewritten(name: &[u8], needle: &[u8], replacement: &[u8], mode: Mode) -> Vec<u8> {
        rewrite(name, needle, replacement, mode, b'/').new
    }

    #[test]
    fn test_first_replaces_only_the_leading_occurrence() {
        assert_eq!(substitute(b"axbxcx", b"x", b"Z", Mode::First), b"aZbxcx");
        assert_eq!(substitute(b"xx", b"x", b"Z", Mode::First), b"Zx");
    }

    #[test]
    fn test_all_replaces_every_occurrence() {
        assert_eq!(substitute(b"axbxcx", b"x", b"Z", Mode::All), b"aZbZcZ");
        assert_eq!(substitute(b"xx", b"x", b"Z", Mode::All), b"ZZ");
    }

    #[test]
    fn test_last_replaces_only_the_trailing_occurrence() {
        assert_eq!(substitute(b"axbxcx", b"x", b"Z", Mode::Last), b"axbxcZ");
        assert_eq!(substitute(b"xx", b"x", b"Z", Mode::Last), b"xZ");
    }

    /// Matches do not overlap: the scan resumes after the match it just took.
    #[test]
    fn test_matches_do_not_overlap() {
        assert_eq!(substitute(b"aaaa", b"aa", b"Z", Mode::First), b"Zaa");
        assert_eq!(substitute(b"aaaa", b"aa", b"Z", Mode::All), b"ZZ");
        assert_eq!(substitute(b"aaaa", b"aa", b"Z", Mode::Last), b"aaZ");
    }

    /// Last scans backward, which an odd-length overlap distinguishes from
    /// "the final match a forward scan happens to reach".
    #[test]
    fn test_last_scans_backward() {
        assert_eq!(substitute(b"aaaa", b"aaa", b"Z", Mode::Last), b"aZ");
        assert_eq!(substitute(b"aaaa", b"aaa", b"Z", Mode::All), b"Za");
    }

    /// The replacement is never rescanned, so a replacement containing the
    /// needle terminates instead of looping.
    #[test]
    fn test_the_replacement_is_not_rescanned() {
        assert_eq!(substitute(b"aaaa", b"a", b"aa", Mode::All), b"aaaaaaaa");
        assert_eq!(substitute(b"x", b"x", b"xx", Mode::All), b"xx");
        assert_eq!(substitute(b"ab", b"ab", b"abab", Mode::All), b"abab");
    }

    #[test]
    fn test_no_match_returns_the_name_unchanged() {
        assert_eq!(substitute(b"n1", b"q", b"z", Mode::First), b"n1");
        assert_eq!(substitute(b"a", b"aaa", b"Z", Mode::All), b"a");
    }

    #[test]
    fn test_an_empty_replacement_deletes() {
        assert_eq!(substitute(b"axbxcx", b"x", b"", Mode::First), b"abxcx");
        assert_eq!(substitute(b"axbxcx", b"x", b"", Mode::All), b"abc");
        assert_eq!(substitute(b"axbxcx", b"x", b"", Mode::Last), b"axbxc");
    }

    #[test]
    fn test_an_empty_needle_prepends_by_default() {
        assert_eq!(substitute(b"abc", b"", b"Z", Mode::First), b"Zabc");
        assert_eq!(substitute(b"x", b"", b"Z", Mode::First), b"Zx");
    }

    #[test]
    fn test_an_empty_needle_appends_under_last() {
        assert_eq!(substitute(b"abc", b"", b"Z", Mode::Last), b"abcZ");
        assert_eq!(substitute(b"x", b"", b"Z", Mode::Last), b"xZ");
    }

    /// n + 1 insertions for a name of n code units, both ends included.
    #[test]
    fn test_an_empty_needle_interleaves_under_all() {
        assert_eq!(substitute(b"abc", b"", b"Z", Mode::All), b"ZaZbZcZ");
        assert_eq!(substitute(b"x", b"", b"Z", Mode::All), b"ZxZ");
        assert_eq!(substitute(b"abc", b"", b"ZY", Mode::All), b"ZYaZYbZYcZY");
    }

    /// The count is in bytes, not characters. A name holding one two-byte
    /// character is five bytes and takes six insertions; a char-based engine
    /// would give five and would be wrong on exactly the mojibake filenames
    /// people reach for rename to fix.
    #[test]
    fn test_the_interleave_counts_code_units_not_characters() {
        assert_eq!(
            substitute(b"caf\xc3\xa9", b"", b"_", Mode::All),
            b"_c_a_f_\xc3_\xa9_"
        );
    }

    /// A needle may split a multibyte sequence; C util-linux does it without
    /// complaint and the result is not valid UTF-8.
    #[test]
    fn test_matching_is_over_bytes_with_no_character_awareness() {
        assert_eq!(
            substitute(b"caf\xc3\xa9", b"\xc3", b"X", Mode::First),
            b"cafX\xa9"
        );
        assert_eq!(substitute(b"lat\xe9n", b"n", b"N", Mode::Last), b"lat\xe9N");
    }

    /// Unreachable through the CLI - an empty operand never survives the
    /// existence check. Pinned so the code cannot panic on it.
    #[test]
    fn test_an_empty_needle_and_an_empty_name_still_insert_once() {
        assert_eq!(substitute(b"", b"", b"Z", Mode::All), b"Z");
        assert_eq!(substitute(b"", b"", b"Z", Mode::First), b"Z");
        assert_eq!(substitute(b"", b"", b"Z", Mode::Last), b"Z");
    }

    /// Either argv string containing a separator switches the whole path into
    /// scope; the check is on the strings, not on the result.
    #[test]
    fn test_a_separator_in_either_argument_widens_the_scope() {
        assert_eq!(rewritten(b"d1/s1", b"s", b"z", Mode::First), b"d1/z1");
        assert_eq!(rewritten(b"d1/s1", b"d1", b"d3", Mode::First), b"d1/s1");
        assert_eq!(rewritten(b"d1/s1", b"d1/", b"d3/", Mode::First), b"d3/s1");
        // A separator in the replacement alone widens the scope just as well,
        // and the substitution stays literal: the separator already in the
        // name is not absorbed, giving the doubled one below.
        assert_eq!(rewritten(b"d1/s1", b"d1", b"d3/", Mode::First), b"d3//s1");
        assert_eq!(rewritten(b"d1/s1", b"d1/s", b"X", Mode::First), b"X1");
    }

    /// Whole-path mode is entered even when the needle cannot match, which
    /// settles "before or after substitution".
    #[test]
    fn test_the_scope_is_decided_before_matching() {
        assert_eq!(rewritten(b"d1/s1", b"z/q", b"Q", Mode::First), b"d1/s1");
    }

    #[test]
    fn test_only_the_final_component_is_rewritten_by_default() {
        assert_eq!(rewritten(b"d1//s1", b"s", b"z", Mode::First), b"d1//z1");
        assert_eq!(rewritten(b"d1/s1", b"", b"X", Mode::First), b"d1/Xs1");
        assert_eq!(rewritten(b"d1/s1", b"", b"X", Mode::All), b"d1/XsX1X");
        assert_eq!(rewritten(b"./top1", b"top", b"TOP", Mode::First), b"./TOP1");
        assert_eq!(rewritten(b"./top1", b".", b"X", Mode::First), b"./top1");
    }

    /// Trailing separators are not part of the component, and they are gone
    /// from the old side too: `d1/` is reported as `d1`.
    #[test]
    fn test_trailing_separators_are_stripped_from_both_sides() {
        let one = rewrite(b"d1/", b"1", b"2", Mode::First, b'/');
        assert_eq!(one.old, b"d1");
        assert_eq!(one.new, b"d2");

        let many = rewrite(b"d1///", b"1", b"2", Mode::First, b'/');
        assert_eq!(many.old, b"d1");
        assert_eq!(many.new, b"d2");

        let nested = rewrite(b"d1/sub/", b"sub", b"SUB", Mode::First, b'/');
        assert_eq!(nested.old, b"d1/sub");
        assert_eq!(nested.new, b"d1/SUB");

        assert_eq!(rewritten(b"d1/", b"", b"Y", Mode::First), b"Yd1");
    }

    /// Stripping happens before the comparison, so an operand whose component
    /// does not change is a no-op even though the raw operand and the new name
    /// differ by a separator. C util-linux exits 4 in silence here.
    #[test]
    fn test_a_component_that_does_not_change_is_unchanged_despite_the_stripping() {
        let quiet = rewrite(b"d1/", b"QQ", b"ZZ", Mode::First, b'/');
        assert_eq!(quiet.old, b"d1");
        assert_eq!(quiet.new, b"d1");
        assert!(quiet.is_unchanged());

        let moved = rewrite(b"d1/", b"1", b"2", Mode::First, b'/');
        assert!(!moved.is_unchanged());
    }

    /// Whole-path mode does no stripping at all - there is no component to
    /// split out - so a trailing separator survives on both sides.
    #[test]
    fn test_whole_path_mode_does_not_strip() {
        let kept = rewrite(b"d1/", b"d1/", b"d9/", Mode::First, b'/');
        assert_eq!(kept.old, b"d1/");
        assert_eq!(kept.new, b"d9/");
    }

    /// A name that is nothing but separators keeps them all: it is not
    /// stripped, and its component is the final separator by itself.
    #[test]
    fn test_an_all_separator_name_keeps_its_separators() {
        let root = rewrite(b"/", b"", b"Y", Mode::First, b'/');
        assert_eq!(root.old, b"/");
        assert_eq!(root.new, b"Y/");

        assert_eq!(rewritten(b"/", b"", b"Y", Mode::All), b"Y/Y");
        assert_eq!(rewritten(b"//", b"", b"Y", Mode::First), b"/Y/");
        assert_eq!(rewritten(b"//", b"", b"Y", Mode::All), b"/Y/Y");
        assert_eq!(rewritten(b"///", b"", b"Y", Mode::First), b"//Y/");
        assert_eq!(rewritten(b"/", b"x", b"y", Mode::First), b"/");
        assert_eq!(rewritten(b"", b"x", b"y", Mode::First), b"");
    }

    /// In whole-path mode the modes apply to the separators themselves.
    #[test]
    fn test_whole_path_mode_treats_separators_as_ordinary_units() {
        assert_eq!(rewritten(b"d1//s1", b"/", b"_", Mode::First), b"d1_/s1");
        assert_eq!(rewritten(b"d1//s1", b"/", b"_", Mode::Last), b"d1/_s1");
        assert_eq!(rewritten(b"d1//s1", b"/", b"_", Mode::All), b"d1__s1");
        assert_eq!(rewritten(b"d1/s1", b"/", b"//", Mode::First), b"d1//s1");
    }

    #[test]
    fn test_the_scope_rule_is_code_unit_generic() {
        let sep = u16::from(b'/');
        let name: Vec<u16> = "d1/s1".encode_utf16().collect();
        let needle: Vec<u16> = "s".encode_utf16().collect();
        let replacement: Vec<u16> = "z".encode_utf16().collect();
        let expected: Vec<u16> = "d1/z1".encode_utf16().collect();
        assert_eq!(
            rewrite(&name, &needle, &replacement, Mode::First, sep).new,
            expected
        );
    }

    /// The engine is instantiated over u16 on Windows, which a unix build
    /// never compiles. Same table, same expectations.
    #[test]
    fn test_the_engine_is_code_unit_generic() {
        let name: Vec<u16> = "axbxcx".encode_utf16().collect();
        let needle: Vec<u16> = "x".encode_utf16().collect();
        let replacement: Vec<u16> = "Z".encode_utf16().collect();
        let expected: Vec<u16> = "aZbZcZ".encode_utf16().collect();
        assert_eq!(
            substitute(&name, &needle, &replacement, Mode::All),
            expected
        );
    }
}
