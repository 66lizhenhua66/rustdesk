use crate::message_proto::OrdAccessRequest;

pub const ACCESS_SCOPE: &str = "windows_primary_view";

pub fn access_proof_payload(
    target_id: &str,
    target_public_key: &[u8],
    challenge: &str,
    request: &OrdAccessRequest,
) -> Result<Vec<u8>, &'static str> {
    if !(6..=20).contains(&target_id.len())
        || !target_id.bytes().all(|b| b.is_ascii_digit())
        || target_public_key.len() != 32
        || challenge.len() != 44
        || !challenge.ends_with('=')
        || !challenge[..43]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
        || request.version != 1
        || request.controller_public_key.len() != 32
        || request.proof.len() != 64
        || request.scope != ACCESS_SCOPE
        || request
            .special_fields
            .unknown_fields()
            .iter()
            .next()
            .is_some()
    {
        return Err("Invalid access request");
    }
    match request.mode.as_str() {
        "pair" if request.credential_id.is_empty() && request.credential_secret.is_empty() => {}
        "unattended"
            if request.credential_id.len() == 16 && request.credential_secret.len() == 32 => {}
        _ => return Err("Invalid access credentials"),
    }
    fn field(out: &mut Vec<u8>, value: &[u8]) {
        out.extend_from_slice(&(value.len() as u32).to_be_bytes());
        out.extend_from_slice(value);
    }
    let mut out = b"Open Remote Desk access v1\0".to_vec();
    field(&mut out, target_id.as_bytes());
    field(&mut out, target_public_key);
    field(&mut out, challenge.as_bytes());
    out.extend_from_slice(&request.version.to_be_bytes());
    for bytes in [
        request.mode.as_bytes(),
        &request.controller_public_key[..],
        &request.credential_id[..],
        &request.credential_secret[..],
        request.scope.as_bytes(),
    ] {
        field(&mut out, bytes);
    }
    Ok(out)
}
