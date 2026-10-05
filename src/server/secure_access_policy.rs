use base::{
    access_proof::access_proof_payload,
    message_proto::{OrdAccessGrant, OrdAccessRequest},
};
use hbb_common::{
    anyhow::Context as _,
    bail,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    sodiumoxide::{crypto::sign, randombytes},
    ResultType,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const LIFE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_STORE: u64 = 256 * 1024;
#[derive(Clone)]
pub struct Context {
    pub target: String,
    pub public: Vec<u8>,
    pub policy: String,
    pub enabled: bool,
    pub two_factor: bool,
}
impl Context {
    pub fn allows(&self) -> ResultType<()> {
        if !self.enabled || self.two_factor {
            bail!("Trusted access disabled or requires unsupported 2FA");
        }
        if self.public.len() != 32 {
            bail!("Invalid target public identity");
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub controller: String,
    pub secret_hash: String,
    pub name: String,
    pub scope: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub target: String,
    pub public: String,
    pub policy: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedRecord {
    record: Record,
    signature: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Store {
    version: u32,
    devices: Vec<SignedRecord>,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            version: 1,
            devices: vec![],
        }
    }
}

pub fn fingerprint(public: &[u8]) -> String {
    format!("{:x}", Sha256::digest(public))
}
pub fn pair_approval_allowed(
    pending: bool,
    authorized: bool,
    explicit_pair: bool,
    local: bool,
) -> bool {
    pending && !authorized && explicit_pair && local
}
pub fn verify_request(
    ctx: &Context,
    challenge: &str,
    request: &OrdAccessRequest,
) -> ResultType<()> {
    ctx.allows()?;
    if STANDARD.decode(challenge)?.len() != 32 {
        bail!("Invalid access challenge");
    }
    let public = sign::PublicKey::from_slice(&request.controller_public_key)
        .context("Invalid controller public key")?;
    let signature = sign::Signature::from_bytes(&request.proof)
        .map_err(|_| hbb_common::anyhow::anyhow!("Invalid proof signature"))?;
    let mut payload = access_proof_payload(&ctx.target, &ctx.public, challenge, request)
        .map_err(hbb_common::anyhow::Error::msg)?;
    let verified = sign::verify_detached(&signature, &payload, &public);
    hbb_common::sodiumoxide::utils::memzero(&mut payload);
    if !verified {
        bail!("Trusted controller proof failed");
    }
    Ok(())
}
fn record_payload(record: &Record) -> ResultType<Vec<u8>> {
    let mut bytes = b"Open Remote Desk stored grant v1\0".to_vec();
    bytes.extend(serde_json::to_vec(record)?);
    Ok(bytes)
}
fn lock(path: &Path) -> ResultType<File> {
    let lock_path = path.with_extension("ord-access-lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    Ok(options
        .open(lock_path)
        .context("Trusted access store is busy or unavailable")?)
}
fn load(path: &Path, ctx: &Context) -> ResultType<Store> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Store::default()),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_STORE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_STORE {
        bail!("Trusted access store too large");
    }
    let store: Store = serde_json::from_slice(&bytes)?;
    if store.version != 1 || store.devices.len() > 64 {
        bail!("Unknown or oversized trusted access store");
    }
    let pk = sign::PublicKey::from_slice(&ctx.public).context("Invalid target key")?;
    for grant in &store.devices {
        let signature = sign::Signature::from_bytes(&STANDARD.decode(&grant.signature)?)
            .map_err(|_| hbb_common::anyhow::anyhow!("Invalid stored signature"))?;
        if !sign::verify_detached(&signature, &record_payload(&grant.record)?, &pk) {
            bail!("Trusted access store signature failed");
        }
    }
    Ok(store)
}
fn save(path: &Path, store: &Store) -> ResultType<()> {
    let bytes = serde_json::to_vec(store)?;
    if bytes.len() as u64 > MAX_STORE {
        bail!("Trusted access store too large");
    }
    let temp = path.with_extension(format!(
        "ord-access-{}.tmp",
        fingerprint(&randombytes::randombytes(16))
    ));
    let result = (|| -> ResultType<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            #[link(name = "kernel32")]
            extern "system" {
                fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
            }
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        #[cfg(not(windows))]
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        if let Err(error) = std::fs::remove_file(&temp) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.into());
            }
        }
    }
    result
}
fn valid(record: &Record, ctx: &Context, now: i64) -> bool {
    record.target == ctx.target
        && record.public == STANDARD.encode(&ctx.public)
        && record.policy == ctx.policy
        && record.scope == "windows_primary_view"
        && record.created_at_ms <= now
        && now < record.expires_at_ms
        && record.expires_at_ms.saturating_sub(record.created_at_ms) <= LIFE_MS
}
pub fn pair(
    path: &Path,
    ctx: &Context,
    secret_key: &[u8],
    controller: &[u8],
    name: &str,
    now: i64,
) -> ResultType<(OrdAccessGrant, Record)> {
    ctx.allows()?;
    if controller.len() != 32 || name.len() > 128 {
        bail!("Invalid controller identity");
    }
    let _lock = lock(path)?;
    let mut store = load(path, ctx)?;
    store.devices.retain(|d| valid(&d.record, ctx, now));
    if store.devices.len() >= 64 {
        bail!("Trusted device limit reached");
    }
    let id = randombytes::randombytes(16);
    let secret = randombytes::randombytes(32);
    let record = Record {
        id: STANDARD.encode(&id),
        controller: STANDARD.encode(controller),
        secret_hash: fingerprint(&secret),
        name: name.into(),
        scope: "windows_primary_view".into(),
        created_at_ms: now,
        expires_at_ms: now.checked_add(LIFE_MS).context("Invalid expiry")?,
        target: ctx.target.clone(),
        public: STANDARD.encode(&ctx.public),
        policy: ctx.policy.clone(),
    };
    let sk = sign::SecretKey::from_slice(secret_key).context("Invalid target signing key")?;
    let signature = STANDARD.encode(sign::sign_detached(&record_payload(&record)?, &sk).as_ref());
    store.devices.push(SignedRecord {
        record: record.clone(),
        signature,
    });
    save(path, &store)?;
    Ok((
        OrdAccessGrant {
            version: 1,
            credential_id: id.into(),
            credential_secret: secret.into(),
            controller_public_key: controller.to_vec().into(),
            expires_at_ms: record.expires_at_ms,
            scope: record.scope.clone(),
            ..Default::default()
        },
        record,
    ))
}
pub fn list(path: &Path, ctx: &Context, now: i64) -> ResultType<Vec<Record>> {
    let _lock = lock(path)?;
    Ok(load(path, ctx)?
        .devices
        .into_iter()
        .map(|d| d.record)
        .filter(|r| valid(r, ctx, now))
        .collect())
}
pub fn active(path: &Path, ctx: &Context, id: &str, now: i64) -> ResultType<Record> {
    ctx.allows()?;
    list(path, ctx, now)?
        .into_iter()
        .find(|r| r.id == id)
        .context("Trusted access expired, revoked, or policy changed")
}
pub fn authorize(
    path: &Path,
    ctx: &Context,
    controller: &[u8],
    id: &[u8],
    secret: &[u8],
    now: i64,
) -> ResultType<Record> {
    let record = active(path, ctx, &STANDARD.encode(id), now)?;
    if record.controller != STANDARD.encode(controller)
        || secret.len() != 32
        || !hbb_common::sodiumoxide::utils::memcmp(
            record.secret_hash.as_bytes(),
            fingerprint(secret).as_bytes(),
        )
    {
        bail!("Trusted credential rejected");
    }
    Ok(record)
}
pub fn revoke(path: &Path, ctx: &Context, id: Option<&str>) -> ResultType<()> {
    let _lock = lock(path)?;
    let Some(id) = id else {
        return save(path, &Store::default());
    };
    let mut store = load(path, ctx)?;
    store.devices.retain(|d| d.record.id != id);
    save(path, &store)
}
