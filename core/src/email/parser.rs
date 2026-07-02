use mailparse::ParsedMail;

pub struct IncomingMail {
    pub encrypted_body: Vec<u8>,
    pub files: Vec<IncomingAttachment>,
}

pub struct IncomingAttachment {
    pub filename: String,
    pub data: Vec<u8>,
}

pub fn parse_incoming(raw: &[u8]) -> Result<IncomingMail, String> {
    let parsed = mailparse::parse_mail(raw).map_err(|e| format!("parse mail failed: {}", e))?;

    let mut encrypted_body = Vec::new();
    let mut files = Vec::new();

    extract_parts(&parsed, &mut encrypted_body, &mut files);

    if encrypted_body.is_empty() {
        return Err("no yse body part found".into());
    }

    // Strip trailing CRLF that MIME parsers may include
    while encrypted_body.ends_with(b"\r\n") || encrypted_body.ends_with(b"\n") {
        let len = if encrypted_body.ends_with(b"\r\n") {
            encrypted_body.len() - 2
        } else {
            encrypted_body.len() - 1
        };
        encrypted_body.truncate(len);
    }

    Ok(IncomingMail {
        encrypted_body,
        files,
    })
}

fn extract_parts(
    part: &ParsedMail,
    encrypted_body: &mut Vec<u8>,
    files: &mut Vec<IncomingAttachment>,
) {
    if part.subparts.is_empty() {
        let content_type = part.ctype.mimetype.as_str();
        match content_type {
            "text/plain" => {
                if let Ok(body) = part.get_body_raw() {
                    if !body.is_empty() {
                        *encrypted_body = body;
                    }
                }
            }
            "application/octet-stream" => {
                let filename = part
                    .get_content_disposition()
                    .params
                    .get("filename")
                    .cloned()
                    .unwrap_or_else(|| "unknown.bin".into());
                if let Ok(data) = part.get_body_raw() {
                    files.push(IncomingAttachment { filename, data });
                }
            }
            _ => {}
        }
    } else {
        for sub in &part.subparts {
            extract_parts(sub, encrypted_body, files);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multipart_mixed() {
        // Use base64 transfer encoding for binary safety
        let raw = b"From: sender@test.com\r\n\
                     To: receiver@test.com\r\n\
                     Subject: \r\n\
                     Content-Type: multipart/mixed; boundary=\"yse_boundary\"\r\n\
                     \r\n\
                     --yse_boundary\r\n\
                     Content-Type: text/plain\r\n\
                     Content-Transfer-Encoding: base64\r\n\
                     \r\n\
                     AQIDBA==\r\n\
                     --yse_boundary\r\n\
                     Content-Type: application/octet-stream\r\n\
                     Content-Disposition: attachment; filename=\"f1.bin\"\r\n\
                     Content-Transfer-Encoding: base64\r\n\
                     \r\n\
                     BQYHCA==\r\n\
                     --yse_boundary--\r\n";

        let parsed = parse_incoming(raw).unwrap();
        assert_eq!(parsed.encrypted_body, vec![1u8, 2, 3, 4]);
        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].filename, "f1.bin");
        assert_eq!(parsed.files[0].data, vec![5u8, 6, 7, 8]);
    }
}
