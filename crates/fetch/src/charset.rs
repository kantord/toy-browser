//! Which encoding a document's bytes are in, and reading them as text.
//!
//! In the order a browser trusts them: a byte-order mark, then the charset the
//! server named, then one the document names for itself in its first kilobyte
//! (`<meta charset>` or the older `http-equiv` form), then UTF-8. A page served
//! as ISO-8859-1 is read as that, so its accents are not replaced by `�`.

use encoding_rs::Encoding;

/// How far into a document a `<meta>` naming its encoding is looked for.
const SNIFFED: usize = 1024;

/// The charset a `Content-Type` header names, if it names one we know.
pub(crate) fn of_header(content_type: &str) -> Option<&'static Encoding> {
    let at = content_type.to_ascii_lowercase().find("charset=")? + "charset=".len();
    let label = content_type[at..]
        .split(';')
        .next()?
        .trim()
        .trim_matches('"');
    Encoding::for_label(label.as_bytes())
}

/// The charset a document's own `<meta>` names, if it names one we know.
fn of_meta(bytes: &[u8]) -> Option<&'static Encoding> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(SNIFFED)]).to_ascii_lowercase();
    let at = head.find("charset")? + "charset".len();
    let label: String = head[at..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=' || c == '"' || c == '\'')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        .collect();
    Encoding::for_label(label.as_bytes())
}

/// `bytes` as text, in the encoding they are most likely in.
pub(crate) fn decode(bytes: &[u8], header: Option<&'static Encoding>) -> String {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return encoding.decode(bytes).0.into_owned();
    }
    let encoding = header
        .or_else(|| of_meta(bytes))
        .unwrap_or(encoding_rs::UTF_8);
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_s_charset_is_used() {
        let header = of_header("text/html; charset=ISO-8859-1");
        assert_eq!(decode(b"Im\xe1genes", header), "Imágenes");
    }

    #[test]
    fn a_document_names_its_own_when_the_server_does_not() {
        let page = b"<meta charset=\"windows-1252\"><p>sesi\xf3n";
        assert!(decode(page, None).ends_with("sesión"));
    }

    #[test]
    fn utf8_is_the_default_and_a_bom_wins() {
        assert_eq!(decode("é".as_bytes(), None), "é");
        let bom = [&[0xEF, 0xBB, 0xBF][..], "é".as_bytes()].concat();
        let latin = of_header("text/html; charset=iso-8859-1");
        assert_eq!(decode(&bom, latin), "é");
    }

    #[test]
    fn a_label_nobody_knows_is_no_charset() {
        assert!(of_header("text/html; charset=nonsense").is_none());
    }
}
