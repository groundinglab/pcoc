//! Minimal S3 SigV4 PUT for R2. Region is `auto`. Credentials stay in the token file.

use crate::timeutil::utc_basic;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

pub struct R2Client {
    account_id: String,
    bucket: String,
    access_key_id: String,
    secret: Zeroizing<String>,
    timeout: Duration,
    http: reqwest::Client,
}

impl R2Client {
    pub fn new(
        account_id: &str,
        bucket: &str,
        token_file: &Path,
        timeout: Duration,
    ) -> Result<Self, String> {
        let (access_key_id, secret) = read_token(token_file)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|err| format!("could not build r2 client: {err}"))?;
        Ok(Self {
            account_id: account_id.to_owned(),
            bucket: bucket.to_owned(),
            access_key_id,
            secret,
            timeout,
            http,
        })
    }

    pub async fn put(&self, key: &str, body: &[u8], content_type: &str) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "clock is before the unix epoch".to_owned())?
            .as_secs() as i64;
        let amz_date = utc_basic(now);
        let day = &amz_date[..8];
        let payload_hash = hex::encode(Sha256::digest(body));
        let host = format!("{}.r2.cloudflarestorage.com", self.account_id);
        let uri = format!("/{}/{}", self.bucket, key.trim_start_matches('/'));
        let headers = [
            ("content-type", content_type),
            ("host", host.as_str()),
            ("x-amz-content-sha256", payload_hash.as_str()),
            ("x-amz-date", amz_date.as_str()),
        ];
        let authorization = sign(SigRequest {
            method: "PUT",
            uri: &uri,
            query: "",
            headers: &headers,
            payload_hash: &payload_hash,
            access_key: &self.access_key_id,
            secret: &self.secret,
            day,
            amz_date: &amz_date,
            region: "auto",
            service: "s3",
        });
        let url = format!("https://{host}{}", encode_uri_path(&uri));
        let response = tokio::time::timeout(
            self.timeout,
            self.http
                .put(url)
                .header("content-type", content_type)
                .header("x-amz-content-sha256", payload_hash)
                .header("x-amz-date", amz_date)
                .header("authorization", authorization)
                .body(body.to_vec())
                .send(),
        )
        .await
        .map_err(|_| format!("r2 put timed out for {key}"))?
        .map_err(|_| format!("r2 put failed for {key}"))?;
        if !response.status().is_success() {
            return Err(format!("r2 put for {key} returned {}", response.status()));
        }
        Ok(())
    }
}

/// Two non-comment lines: access key id, then secret. Mode must hide the file
/// from group and other.
pub fn read_token(path: &Path) -> Result<(String, Zeroizing<String>), String> {
    let meta = fs::metadata(path).map_err(|err| format!("could not stat token file: {err}"))?;
    if !meta.is_file() {
        return Err("token file is not a regular file".into());
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(format!(
            "token file mode is {mode:04o}; group and other must have no access"
        ));
    }
    let raw =
        fs::read_to_string(path).map_err(|err| format!("could not read token file: {err}"))?;
    if raw.len() > 4096 {
        return Err("token file is too long".into());
    }
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if lines.len() != 2 || lines[0].is_empty() || lines[1].is_empty() {
        return Err("token file needs an access key id and a secret".into());
    }
    Ok((lines[0].to_owned(), Zeroizing::new(lines[1].to_owned())))
}

struct SigRequest<'a> {
    method: &'a str,
    uri: &'a str,
    query: &'a str,
    headers: &'a [(&'a str, &'a str)],
    payload_hash: &'a str,
    access_key: &'a str,
    secret: &'a str,
    day: &'a str,
    amz_date: &'a str,
    region: &'a str,
    service: &'a str,
}

fn sign(request: SigRequest<'_>) -> String {
    let SigRequest {
        method,
        uri,
        query,
        headers,
        payload_hash,
        access_key,
        secret,
        day,
        amz_date,
        region,
        service,
    } = request;
    let canonical_uri = encode_uri_path(uri);
    let mut pairs: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    let mut canonical_headers = String::new();
    for (name, value) in &pairs {
        canonical_headers.push_str(name);
        canonical_headers.push(':');
        canonical_headers.push_str(value);
        canonical_headers.push('\n');
    }
    let signed_headers = pairs
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical = format!(
        "{method}\n{canonical_uri}\n{query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let scope = format!("{day}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let signing_key = signing_key(secret, day, region, service);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    format!("AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}")
}

fn signing_key(secret: &str, day: &str, region: &str, service: &str) -> [u8; 32] {
    let date_key = hmac_sha256(format!("AWS4{secret}").as_bytes(), day.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, service.as_bytes());
    hmac_sha256(&service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn encode_uri_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_get_object_vector() {
        // AWS documentation sample, GET Object, 2013-05-24.
        let payload = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let headers = [
            ("host", "examplebucket.s3.amazonaws.com"),
            ("range", "bytes=0-9"),
            ("x-amz-content-sha256", payload),
            ("x-amz-date", "20130524T000000Z"),
        ];
        let header = sign(SigRequest {
            method: "GET",
            uri: "/test.txt",
            query: "",
            headers: &headers,
            payload_hash: payload,
            access_key: "AKIAIOSFODNN7EXAMPLE",
            secret: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            day: "20130524",
            amz_date: "20130524T000000Z",
            region: "us-east-1",
            service: "s3",
        });
        assert!(
            header.ends_with("f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"),
            "{header}"
        );
    }
}
