use crate::{
    access_proof::{access_proof_payload, ACCESS_SCOPE},
    protos::message::{OrdAccessGrant, OrdAccessRequest},
    session::ControllerSessionCallback,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use sodiumoxide::crypto::sign;
use std::{
    ffi::{c_char, c_void, CString},
    ptr,
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Secrets {
    mode: String,
    seed: String,
    credential_id: String,
    credential_secret: String,
}
impl Drop for Secrets {
    fn drop(&mut self) {
        self.seed.zeroize();
        self.credential_secret.zeroize();
    }
}
pub(crate) struct Access {
    pub(crate) mode: String,
    public_key: sign::PublicKey,
    seed: Zeroizing<Vec<u8>>,
    credential_id: Vec<u8>,
    credential_secret: Zeroizing<Vec<u8>>,
    granted: bool,
}
impl Access {
    pub(crate) fn parse(input: &str) -> Option<Self> {
        if input.len() > 2048 {
            return None;
        }
        let secrets = serde_json::from_str::<Secrets>(input).ok()?;
        let seed = Zeroizing::new(STANDARD.decode(&secrets.seed).ok()?);
        let signing_seed = sign::Seed::from_slice(&seed)?;
        let (public_key, _) = sign::keypair_from_seed(&signing_seed);
        let credential_id = STANDARD.decode(&secrets.credential_id).ok()?;
        let credential_secret = Zeroizing::new(STANDARD.decode(&secrets.credential_secret).ok()?);
        if !matches!(secrets.mode.as_str(), "pair" | "unattended")
            || (secrets.mode == "pair"
                && (!credential_id.is_empty() || !credential_secret.is_empty()))
            || (secrets.mode == "unattended"
                && (credential_id.len() != 16 || credential_secret.len() != 32))
        {
            return None;
        }
        Some(Self {
            mode: secrets.mode.clone(),
            public_key,
            seed,
            credential_id,
            credential_secret,
            granted: false,
        })
    }
    pub(crate) fn proof(
        &mut self,
        target_id: &str,
        target_key: &sign::PublicKey,
        challenge: &str,
    ) -> Result<OrdAccessRequest, &'static str> {
        if STANDARD.decode(challenge).map_or(true, |b| b.len() != 32) {
            return Err("ACCESS_CHALLENGE_INVALID");
        }
        let seed = sign::Seed::from_slice(&self.seed).ok_or("ACCESS_PROOF_FAILED")?;
        let (_, secret) = sign::keypair_from_seed(&seed);
        let mut request = OrdAccessRequest {
            version: 1,
            mode: self.mode.clone(),
            controller_public_key: self.public_key.0.to_vec(),
            credential_id: self.credential_id.clone(),
            credential_secret: self.credential_secret.to_vec(),
            proof: vec![0; 64],
            scope: ACCESS_SCOPE.into(),
            ..Default::default()
        };
        let payload = Zeroizing::new(
            access_proof_payload(target_id, &target_key.0, challenge, &request)
                .map_err(|_| "ACCESS_PROOF_FAILED")?,
        );
        request.proof = sign::sign_detached(&payload, &secret).to_bytes().to_vec();
        self.seed.zeroize();
        self.credential_secret.zeroize();
        Ok(request)
    }
    pub(crate) fn grant(&mut self, grant: &OrdAccessGrant) -> Result<(), &'static str> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "ACCESS_GRANT_INVALID")?
            .as_millis() as i64;
        if self.mode != "pair"
            || self.granted
            || grant.version != 1
            || grant.scope != ACCESS_SCOPE
            || grant.controller_public_key != self.public_key.0
            || grant.credential_id.len() != 16
            || grant.credential_secret.len() != 32
            || grant.expires_at_ms <= now
            || grant.expires_at_ms > now + 31 * 24 * 60 * 60 * 1000
            || grant
                .special_fields
                .unknown_fields()
                .iter()
                .next()
                .is_some()
        {
            return Err("ACCESS_GRANT_INVALID");
        }
        self.granted = true;
        Ok(())
    }
    pub(crate) fn verify_peer_info(&self, additions: &str) -> Result<(), &'static str> {
        let value: serde_json::Value =
            serde_json::from_str(additions).map_err(|_| "ACCESS_MODE_MISMATCH")?;
        if value.get("access_mode").and_then(|v| v.as_str()) != Some(self.mode.as_str())
            || (self.mode == "pair" && !self.granted)
        {
            return Err("ACCESS_MODE_MISMATCH");
        }
        Ok(())
    }
}
pub(crate) fn paired_event(
    callback: Option<ControllerSessionCallback>,
    user: *mut c_void,
    grant: &OrdAccessGrant,
) {
    if let Some(callback) = callback {
        let secret = Zeroizing::new(STANDARD.encode(&grant.credential_secret));
        let event=Zeroizing::new(format!("{{\"state\":\"access_paired\",\"code\":\"ACCESS_PAIRED\",\"message\":\"Read-only access paired\",\"verified\":true,\"authenticated\":false,\"authorized\":false,\"credentialId\":\"{}\",\"credentialSecret\":\"{}\",\"expiresAt\":{}}}\0",STANDARD.encode(&grant.credential_id),secret.as_str(),grant.expires_at_ms));
        unsafe { callback(event.as_ptr().cast(), user) }
    }
}
#[no_mangle]
pub extern "C" fn controller_identity_create_v1() -> *mut c_char {
    if sodiumoxide::init().is_err() {
        return ptr::null_mut();
    }
    let mut seed = sign::Seed([0; 32]);
    sodiumoxide::randombytes::randombytes_into(&mut seed.0);
    let (public, _) = sign::keypair_from_seed(&seed);
    let seed_text = Zeroizing::new(STANDARD.encode(&seed.0));
    let value = Zeroizing::new(format!(
        "{{\"seed\":\"{}\",\"publicKey\":\"{}\"}}",
        seed_text.as_str(),
        STANDARD.encode(public.0)
    ));
    CString::new(value.as_bytes()).map_or(ptr::null_mut(), CString::into_raw)
}
#[no_mangle]
pub extern "C" fn controller_secret_string_free_v1(value: *mut c_char) {
    if !value.is_null() {
        let value = unsafe { CString::from_raw(value) };
        let mut bytes = value.into_bytes_with_nul();
        bytes.zeroize();
    }
}
