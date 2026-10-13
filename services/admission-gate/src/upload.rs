//! Upload body: raw `application/cbor` or a `multipart/form-data` form carrying
//! exactly one file in field `object`.
//!
//! One file. `application/cbor` and `application/octet-stream` are accepted.
//! More than one file is rejected. The byte cap is the process transport limit.
//!
//! The multipart reader here is deliberately small: it handles the single-file
//! form a browser or `curl -F` sends and refuses anything it does not
//! understand. It is not a general RFC 7578 implementation.

#[derive(Debug, PartialEq, Eq)]
pub enum UploadError {
    Empty,
    UnsupportedMediaType(String),
    Oversize { bytes: usize, limit: usize },
    MultiFile { files: usize },
    BadMultipart(String),
    MissingObjectField,
}

impl UploadError {
    pub fn code(&self) -> &'static str {
        match self {
            UploadError::Empty => "empty_body",
            UploadError::UnsupportedMediaType(_) => "unsupported_media_type",
            UploadError::Oversize { .. } => "oversize",
            UploadError::MultiFile { .. } => "multi_file",
            UploadError::BadMultipart(_) => "bad_multipart",
            UploadError::MissingObjectField => "missing_object_field",
        }
    }

    pub fn message(&self) -> String {
        match self {
            UploadError::Empty => "request body is empty; send one CBOR object".to_owned(),
            UploadError::UnsupportedMediaType(ct) => format!(
                "content-type {ct:?} is not accepted; use application/cbor, \
                 application/octet-stream, or multipart/form-data with field \"object\""
            ),
            UploadError::Oversize { bytes, limit } => format!(
                "object is {bytes} bytes; the door accepts at most {limit} bytes ({:.0} MiB)",
                *limit as f64 / (1024.0 * 1024.0)
            ),
            UploadError::MultiFile { files } => {
                format!("{files} files in this upload; one object is verified per job")
            }
            UploadError::BadMultipart(why) => format!("multipart/form-data body rejected: {why}"),
            UploadError::MissingObjectField => {
                "multipart/form-data body has no file field named \"object\"".to_owned()
            }
        }
    }
}

/// Media types accepted as a raw (non-multipart) object body. An absent
/// content-type is also accepted so `curl --data-binary @obj.cbor` works.
fn raw_body_accepted(mime: &str) -> bool {
    matches!(
        mime,
        "" | "application/cbor"
            | "application/octet-stream"
            | "application/x-cbor"
            | "application/vnd.pcoc.object+cbor"
    )
}

fn mime_of(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Pull the single CBOR object out of a verify request body.
pub fn extract_object(
    content_type: Option<&str>,
    body: &[u8],
    max_object_bytes: usize,
) -> Result<Vec<u8>, UploadError> {
    let content_type = content_type.unwrap_or("").trim();
    let mime = mime_of(content_type);

    let object = if mime == "multipart/form-data" {
        extract_multipart_object(content_type, body)?
    } else if raw_body_accepted(&mime) {
        body.to_vec()
    } else {
        return Err(UploadError::UnsupportedMediaType(mime));
    };

    if object.is_empty() {
        return Err(UploadError::Empty);
    }
    if object.len() > max_object_bytes {
        return Err(UploadError::Oversize {
            bytes: object.len(),
            limit: max_object_bytes,
        });
    }
    Ok(object)
}

struct Part {
    name: String,
    is_file: bool,
    body: Vec<u8>,
}

fn extract_multipart_object(content_type: &str, body: &[u8]) -> Result<Vec<u8>, UploadError> {
    let boundary = boundary_of(content_type)
        .ok_or_else(|| UploadError::BadMultipart("no boundary parameter".to_owned()))?;
    let parts = split_parts(body, &boundary)?;

    let files = parts.iter().filter(|p| p.is_file).count();
    if files > 1 {
        return Err(UploadError::MultiFile { files });
    }
    let named: Vec<&Part> = parts.iter().filter(|p| p.name == "object").collect();
    if named.len() > 1 {
        return Err(UploadError::MultiFile { files: named.len() });
    }
    if let Some(part) = named.first() {
        return Ok(part.body.clone());
    }
    // A form that sent one file under a different field name still has an
    // unambiguous object; anything else is a client bug worth naming.
    match parts.iter().find(|p| p.is_file) {
        Some(part) => Ok(part.body.clone()),
        None => Err(UploadError::MissingObjectField),
    }
}

fn boundary_of(content_type: &str) -> Option<String> {
    for param in content_type.split(';').skip(1) {
        let (key, value) = param.split_once('=')?;
        if key.trim().eq_ignore_ascii_case("boundary") {
            let value = value.trim().trim_matches('"');
            if value.is_empty() {
                return None;
            }
            return Some(value.to_owned());
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

fn bad(why: &str) -> UploadError {
    UploadError::BadMultipart(why.to_owned())
}

fn split_parts(body: &[u8], boundary: &str) -> Result<Vec<Part>, UploadError> {
    let delim = format!("--{boundary}").into_bytes();
    let mut cursor =
        find(body, &delim, 0).ok_or_else(|| bad("opening boundary not found"))? + delim.len();

    let mut parts = Vec::new();
    loop {
        let rest = body.get(cursor..).ok_or_else(|| bad("body truncated"))?;
        if rest.starts_with(b"--") {
            return Ok(parts);
        }
        cursor += skip_eol(rest).ok_or_else(|| bad("boundary not followed by CRLF"))?;

        let (headers_end, body_start) = find(body, b"\r\n\r\n", cursor)
            .map(|i| (i, i + 4))
            .or_else(|| find(body, b"\n\n", cursor).map(|i| (i, i + 2)))
            .ok_or_else(|| bad("part headers unterminated"))?;
        let headers = std::str::from_utf8(&body[cursor..headers_end])
            .map_err(|_| bad("part headers are not UTF-8"))?;
        let (name, is_file) = parse_disposition(headers)?;

        let next =
            find(body, &delim, body_start).ok_or_else(|| bad("closing boundary not found"))?;
        // The delimiter is preceded by the CRLF that terminates the part body.
        let mut body_end = next;
        if body[..body_end].ends_with(b"\r\n") {
            body_end -= 2;
        } else if body[..body_end].ends_with(b"\n") {
            body_end -= 1;
        }
        if body_end < body_start {
            return Err(bad("part body shorter than its delimiter"));
        }

        parts.push(Part {
            name,
            is_file,
            body: body[body_start..body_end].to_vec(),
        });
        cursor = next + delim.len();
    }
}

fn skip_eol(rest: &[u8]) -> Option<usize> {
    if rest.starts_with(b"\r\n") {
        Some(2)
    } else if rest.starts_with(b"\n") {
        Some(1)
    } else {
        None
    }
}

/// Returns `(field name, looks like a file part)`.
fn parse_disposition(headers: &str) -> Result<(String, bool), UploadError> {
    for line in headers.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("content-disposition") {
            continue;
        }
        let mut name = None;
        let mut filename = None;
        for param in value.split(';').skip(1) {
            if let Some((k, v)) = param.split_once('=') {
                let v = v.trim().trim_matches('"').to_owned();
                match k.trim().to_ascii_lowercase().as_str() {
                    "name" => name = Some(v),
                    "filename" | "filename*" => filename = Some(v),
                    _ => {}
                }
            }
        }
        return Ok((name.unwrap_or_default(), filename.is_some()));
    }
    Err(bad("part has no content-disposition header"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 2 * 1024 * 1024;

    fn form(parts: &[(&str, Option<&str>, &[u8])]) -> (String, Vec<u8>) {
        let boundary = "----glverifyBOUNDARY";
        let mut body = Vec::new();
        for (name, filename, bytes) in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            let disposition = match filename {
                Some(f) => format!("form-data; name=\"{name}\"; filename=\"{f}\""),
                None => format!("form-data; name=\"{name}\""),
            };
            body.extend_from_slice(format!("Content-Disposition: {disposition}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Type: application/cbor\r\n\r\n");
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    #[test]
    fn raw_cbor_body_passes_through() {
        let got = extract_object(Some("application/cbor"), b"\xa1\x01\x02", LIMIT).unwrap();
        assert_eq!(got, b"\xa1\x01\x02");
    }

    #[test]
    fn octet_stream_and_absent_content_type_are_accepted() {
        assert!(extract_object(Some("application/octet-stream"), b"\x01", LIMIT).is_ok());
        assert!(extract_object(None, b"\x01", LIMIT).is_ok());
    }

    #[test]
    fn json_content_type_is_refused() {
        let err = extract_object(Some("application/json"), b"{}", LIMIT).unwrap_err();
        assert_eq!(err.code(), "unsupported_media_type");
    }

    #[test]
    fn empty_body_is_refused() {
        assert_eq!(
            extract_object(Some("application/cbor"), b"", LIMIT).unwrap_err(),
            UploadError::Empty
        );
    }

    #[test]
    fn oversize_object_is_refused() {
        let err = extract_object(Some("application/cbor"), &[0u8; 64], 32).unwrap_err();
        assert_eq!(err.code(), "oversize");
    }

    #[test]
    fn multipart_object_field_is_found() {
        let (ct, body) = form(&[("object", Some("obj.cbor"), b"\xa1\x01\x02")]);
        assert_eq!(
            extract_object(Some(&ct), &body, LIMIT).unwrap(),
            b"\xa1\x01\x02"
        );
    }

    #[test]
    fn multipart_ignores_plain_text_fields() {
        let (ct, body) = form(&[
            ("note", None, b"hello"),
            ("object", Some("obj.cbor"), b"\x01\x02\x03"),
        ]);
        assert_eq!(
            extract_object(Some(&ct), &body, LIMIT).unwrap(),
            b"\x01\x02\x03"
        );
    }

    #[test]
    fn multipart_two_files_is_multi_file() {
        let (ct, body) = form(&[
            ("object", Some("a.cbor"), b"\x01"),
            ("extra", Some("b.cbor"), b"\x02"),
        ]);
        let err = extract_object(Some(&ct), &body, LIMIT).unwrap_err();
        assert_eq!(err.code(), "multi_file");
    }

    #[test]
    fn multipart_two_object_fields_is_multi_file() {
        let (ct, body) = form(&[
            ("object", Some("a.cbor"), b"\x01"),
            ("object", Some("b.cbor"), b"\x02"),
        ]);
        let err = extract_object(Some(&ct), &body, LIMIT).unwrap_err();
        assert_eq!(err.code(), "multi_file");
    }

    #[test]
    fn multipart_single_file_under_other_name_still_verifies() {
        let (ct, body) = form(&[("file", Some("a.cbor"), b"\x07")]);
        assert_eq!(extract_object(Some(&ct), &body, LIMIT).unwrap(), b"\x07");
    }

    #[test]
    fn multipart_without_any_file_is_missing_object() {
        let (ct, body) = form(&[("note", None, b"hello")]);
        let err = extract_object(Some(&ct), &body, LIMIT).unwrap_err();
        assert_eq!(err.code(), "missing_object_field");
    }

    #[test]
    fn multipart_without_boundary_is_bad_multipart() {
        let err = extract_object(Some("multipart/form-data"), b"whatever", LIMIT).unwrap_err();
        assert_eq!(err.code(), "bad_multipart");
    }

    #[test]
    fn multipart_body_preserves_bytes_that_look_like_boundaries() {
        let payload: Vec<u8> = b"\r\n--not-the-boundary\r\nstill object\x00\xff".to_vec();
        let (ct, body) = form(&[("object", Some("a.cbor"), payload.as_slice())]);
        assert_eq!(extract_object(Some(&ct), &body, LIMIT).unwrap(), payload);
    }
}
