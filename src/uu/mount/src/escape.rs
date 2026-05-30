// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

/// Expand octal escape sequences of the form `\NNN` used in mount-table style
/// files to encode whitespace and other special characters.
pub(crate) fn unescape_octal(s: &str) -> String {
    let mut result: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let (a, b, c) = (bytes[i + 1], bytes[i + 2], bytes[i + 3]);
            if a.is_ascii_digit()
                && a < b'8'
                && b.is_ascii_digit()
                && b < b'8'
                && c.is_ascii_digit()
                && c < b'8'
            {
                let value = (a - b'0') * 64 + (b - b'0') * 8 + (c - b'0');
                result.push(value);
                i += 4;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
