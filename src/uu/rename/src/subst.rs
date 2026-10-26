// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

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

    /// An empty operand never survives the existence check, but under -s the
    /// name is a link's target, and macOS lets a link with an empty one exist.
    /// Pinned so the code cannot panic on it.
    #[test]
    fn test_an_empty_needle_and_an_empty_name_still_insert_once() {
        assert_eq!(substitute(b"", b"", b"Z", Mode::All), b"Z");
        assert_eq!(substitute(b"", b"", b"Z", Mode::First), b"Z");
        assert_eq!(substitute(b"", b"", b"Z", Mode::Last), b"Z");
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
}
