use crate::error::{Result, TfError};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use roxmltree::{Document, Node};
use x509_cert::Certificate;
use x509_cert::der::asn1::{AnyRef, OctetStringRef, UintRef};
use x509_cert::der::{Decode, Reader, SliceReader};

fn invalid(code: &str) -> TfError {
    TfError::staged(
        "keybox",
        code,
        "Keybox 格式校验失败，保留旧文件 [Invalid keybox; existing files preserved]",
    )
}

fn children<'a>(node: Node<'a, 'a>, name: &str) -> Vec<Node<'a, 'a>> {
    node.children()
        .filter(|n| {
            n.is_element() && n.tag_name().name() == name && n.tag_name().namespace().is_none()
        })
        .collect()
}

fn one<'a>(node: Node<'a, 'a>, name: &str) -> Result<Node<'a, 'a>> {
    let found = children(node, name);
    if found.len() != 1 {
        return Err(invalid("xml_structure"));
    }
    Ok(found[0])
}

fn shape(node: Node<'_, '_>, allowed: &[&str]) -> Result<()> {
    if node.children().any(|n| {
        (n.is_element()
            && (n.tag_name().namespace().is_some() || !allowed.contains(&n.tag_name().name())))
            || (n.is_text() && !n.text().unwrap_or_default().trim().is_empty())
    }) {
        return Err(invalid("xml_structure"));
    }
    Ok(())
}

fn count(node: Node<'_, '_>, name: &str, actual: usize) -> Result<()> {
    let count = one(node, name)?;
    if count.children().any(|n| n.is_element()) {
        return Err(invalid("xml_count"));
    }
    let declared = count.text().and_then(|s| s.trim().parse::<usize>().ok());
    if actual == 0 || declared != Some(actual) {
        return Err(invalid("xml_count"));
    }
    Ok(())
}

fn pem(node: Node<'_, '_>, allowed: &[&str]) -> Result<(String, Vec<u8>)> {
    if node.attribute("format") != Some("pem") || node.children().any(|n| n.is_element()) {
        return Err(invalid("pem_format"));
    }
    let text = node.text().unwrap_or_default().trim();
    for label in allowed {
        let begin = format!("-----BEGIN {label}-----");
        let end = format!("-----END {label}-----");
        if let Some(body) = text.strip_prefix(&begin).and_then(|s| s.strip_suffix(&end)) {
            let body: String = body.chars().filter(|c| !c.is_ascii_whitespace()).collect();
            let der = STANDARD.decode(body).map_err(|_| invalid("pem_encoding"))?;
            if der.is_empty() {
                return Err(invalid("pem_encoding"));
            }
            return Ok(((*label).into(), der));
        }
    }
    Err(invalid("pem_label"))
}

fn private_der(data: &[u8], rsa: bool) -> x509_cert::der::Result<()> {
    let mut reader = SliceReader::new(data)?;
    reader.sequence(|reader| {
        let version = reader.decode::<u8>()?;
        if version != if rsa { 0 } else { 1 } {
            return Err(reader.error(x509_cert::der::ErrorKind::Failed));
        }
        if rsa {
            for _ in 0..8 {
                reader.decode::<UintRef<'_>>()?;
            }
        } else {
            let key = reader.decode::<OctetStringRef<'_>>()?;
            if key.as_bytes().is_empty() {
                return Err(reader.error(x509_cert::der::ErrorKind::Failed));
            }
            while !reader.is_finished() {
                reader.decode::<AnyRef<'_>>()?;
            }
        }
        Ok(())
    })?;
    reader.finish(())
}

/// 仅验证结构和编码；不把格式有效等同于信任、未吊销或后端验收。
/// Validate structure/encoding only, not trust, revocation or backend acceptance.
pub(crate) fn validate(data: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(data).map_err(|_| invalid("xml_utf8"))?;
    let doc = Document::parse(text).map_err(|_| invalid("xml_parse"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "AndroidAttestation" || root.tag_name().namespace().is_some() {
        return Err(invalid("xml_root"));
    }
    let boxes = children(root, "Keybox");
    shape(root, &["NumberOfKeyboxes", "Keybox"])?;
    count(root, "NumberOfKeyboxes", boxes.len())?;
    for keybox in boxes {
        shape(keybox, &["Key"])?;
        if keybox
            .attribute("DeviceID")
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid("xml_device_id"));
        }
        let keys = children(keybox, "Key");
        if keys.is_empty() {
            return Err(invalid("xml_keys"));
        }
        let mut algorithms = Vec::new();
        for key in keys {
            shape(key, &["PrivateKey", "CertificateChain"])?;
            let algorithm = key.attribute("algorithm").unwrap_or_default();
            if !matches!(algorithm, "rsa" | "ecdsa") || algorithms.contains(&algorithm) {
                return Err(invalid("xml_algorithm"));
            }
            algorithms.push(algorithm);
            let rsa = algorithm == "rsa";
            let labels = if rsa {
                ["RSA PRIVATE KEY", "PRIVATE KEY"]
            } else {
                ["EC PRIVATE KEY", "PRIVATE KEY"]
            };
            let (label, bytes) = pem(one(key, "PrivateKey")?, &labels)?;
            if label == "PRIVATE KEY" {
                let info = p256::pkcs8::PrivateKeyInfo::from_der(&bytes)
                    .map_err(|_| invalid("private_key_der"))?;
                let expected = if rsa {
                    "1.2.840.113549.1.1.1"
                } else {
                    "1.2.840.10045.2.1"
                };
                if info.algorithm.oid.to_string() != expected {
                    return Err(invalid("private_key_algorithm"));
                }
                private_der(info.private_key, rsa).map_err(|_| invalid("private_key_der"))?;
            } else {
                private_der(&bytes, rsa).map_err(|_| invalid("private_key_der"))?;
            }
            let chain = one(key, "CertificateChain")?;
            shape(chain, &["NumberOfCertificates", "Certificate"])?;
            let certificates = children(chain, "Certificate");
            count(chain, "NumberOfCertificates", certificates.len())?;
            for certificate in certificates {
                let (_, bytes) = pem(certificate, &["CERTIFICATE"])?;
                Certificate::from_der(&bytes).map_err(|_| invalid("certificate_der"))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = include_str!("../../../tests/fixtures/keybox-valid.xml");

    #[test]
    fn accepts_multiple_algorithms_and_keyboxes() {
        validate(VALID.as_bytes()).unwrap();
        let start = VALID.find("<Keybox ").unwrap();
        let end = VALID.find("</Keybox>").unwrap() + "</Keybox>".len();
        let multi = VALID
            .replace("<NumberOfKeyboxes>1", "<NumberOfKeyboxes>2")
            .replace(
                "</AndroidAttestation>",
                &format!("{}</AndroidAttestation>", &VALID[start..end]),
            );
        validate(multi.as_bytes()).unwrap();
    }

    #[test]
    fn rejects_broken_structure_counts_and_pem_without_echoing_material() {
        let mutations = [
            VALID.replace("</AndroidAttestation>", ""),
            VALID.replace("<NumberOfKeyboxes>1", "<NumberOfKeyboxes>2"),
            VALID.replace("<NumberOfCertificates>1", "<NumberOfCertificates>2"),
            VALID.replace("PrivateKey", "MissingKey"),
            VALID.replace("-----BEGIN CERTIFICATE-----", "-----BEGIN PUBLIC KEY-----"),
            VALID.replace("-----END CERTIFICATE-----", "!-----END CERTIFICATE-----"),
            VALID.replace("DeviceID=\"synthetic-test\"", ""),
            VALID.replace("algorithm=\"rsa\"", "algorithm=\"ecdsa\""),
            format!("<html><!-- AndroidAttestation -->{VALID}</html>"),
            "<AndroidAttestation>marker-only</AndroidAttestation>".into(),
            format!("{VALID}\0"),
        ];
        for data in mutations {
            let error = validate(data.as_bytes()).expect_err("reject malformed fixture");
            assert_eq!(error.stage(), Some("keybox"));
            assert!(!error.to_string().contains("BEGIN"));
        }
    }
}
