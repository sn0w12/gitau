/// Light magic-byte sniffing; some CDNs serve avatars as generic octet
/// streams, so we trust bytes over headers.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if sniff_svg(bytes) {
        return Some("image/svg+xml");
    }
    if bytes.len() < 12 {
        return None;
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// SVG is text, so sniff for an `<svg` tag after an optional XML
/// declaration, BOM, and leading whitespace.
fn sniff_svg(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let head = &bytes[start..];
    if head.starts_with(b"<svg") {
        return true;
    }
    if !head.starts_with(b"<?xml") {
        return false;
    }
    let window = &head[..head.len().min(512)];
    window
        .to_ascii_lowercase()
        .windows(4)
        .any(|window| window == b"<svg")
}

/// Encodes image bytes as a data URL the webview can use in `<img src>`.
pub fn encode_data_url(content_type: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!(
        "data:{content_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_BYTES: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    ];

    #[test]
    fn sniffs_supported_formats() {
        let webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ".to_vec();
        assert_eq!(sniff_image(PNG_BYTES), Some("image/png"));
        assert_eq!(
            sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some("image/jpeg")
        );
        assert_eq!(sniff_image(b"GIF89axxxxxxx"), Some("image/gif"));
        assert_eq!(
            sniff_image(&[0x00, 0x00, 0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some("image/x-icon")
        );
        assert_eq!(sniff_image(&webp), Some("image/webp"));
        assert_eq!(sniff_image(b"<html>not an img"), None);
    }

    #[test]
    fn sniffs_svg() {
        assert_eq!(
            sniff_image(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some("image/svg+xml")
        );
        assert_eq!(
            sniff_image(b"\xEF\xBB\xBF  <?xml version=\"1.0\"?><svg viewBox=\"0 0 1 1\"/>"),
            Some("image/svg+xml")
        );
        assert_eq!(sniff_image(b"<?xml version=\"1.0\"?><HTML></HTML>"), None);
        assert_eq!(sniff_image(b"not svg at all"), None);
    }
}
