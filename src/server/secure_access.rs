use super::secure_access_policy::{self as policy, Context, Record};
use base::message_proto::{OrdAccessGrant, OrdAccessRequest};
use hbb_common::{
    anyhow::Context as _,
    bail,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    config::{Config, Config2},
    sodiumoxide, ResultType,
};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn enabled() -> bool {
    std::env::var("ORD_SECURE_UNATTENDED").as_deref() == Ok("1")
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
pub fn challenge() -> String {
    STANDARD.encode(sodiumoxide::randombytes::randombytes(32))
}
fn path() -> PathBuf {
    std::env::var_os("ORD_SECURE_ACCESS_STORE")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| Config::file().with_extension("ord-access.json"))
}
fn disk(path: PathBuf) -> ResultType<hbb_common::toml::Value> {
    match std::fs::read_to_string(path) {
        Ok(value) => Ok(hbb_common::toml::from_str(&value)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(hbb_common::toml::Value::Table(Default::default()))
        }
        Err(error) => Err(error.into()),
    }
}
fn context() -> ResultType<Context> {
    if sodiumoxide::init().is_err() {
        bail!("Crypto initialization failed");
    }
    let local = disk(Config::file())?;
    let options = disk(Config2::file())?;
    let disk_2fa = options
        .get("options")
        .and_then(|o| o.get("2fa"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let memory_2fa = Config::get_option("2fa");
    // Include persisted policy so revocation by another process does not wait for Config refresh.
    let selected = serde_json::json!([
        local.get("password"),
        local.get("salt"),
        local.get("key_pair"),
        local.get("enc_id"),
        Config::get_local_permanent_password_storage_and_salt(),
        Config::get_preset_password_storage_and_salt(),
        disk_2fa,
        memory_2fa,
    ]);
    Ok(Context {
        target: Config::get_id(),
        public: Config::get_key_pair().1,
        policy: policy::fingerprint(&serde_json::to_vec(&selected)?),
        enabled: enabled(),
        two_factor: !disk_2fa.is_empty() || !memory_2fa.is_empty(),
    })
}
pub fn prepare(request: &OrdAccessRequest, challenge: &str) -> ResultType<Option<Record>> {
    let ctx = context()?;
    policy::verify_request(&ctx, challenge, request)?;
    match request.mode.as_str() {
        "pair" => Ok(None),
        "unattended" => Ok(Some(policy::authorize(
            &path(),
            &ctx,
            &request.controller_public_key,
            &request.credential_id,
            &request.credential_secret,
            now(),
        )?)),
        _ => bail!("Unknown trusted access mode"),
    }
}
pub fn approve_pair(
    request: &OrdAccessRequest,
    challenge: &str,
    name: &str,
) -> ResultType<(OrdAccessGrant, Record)> {
    let ctx = context()?;
    policy::verify_request(&ctx, challenge, request)?;
    if request.mode != "pair" {
        bail!("Expected pairing request");
    }
    policy::pair(
        &path(),
        &ctx,
        &Config::get_key_pair().0,
        &request.controller_public_key,
        name,
        now(),
    )
}
pub fn active(id: &str) -> ResultType<()> {
    policy::active(&path(), &context()?, id, now()).map(|_| ())
}
pub fn fingerprint(request: &OrdAccessRequest) -> String {
    policy::fingerprint(&request.controller_public_key)
}

pub fn manage(command: &str, value: &str) -> String {
    let result = (|| -> ResultType<serde_json::Value> {
        let ctx = context()?;
        match command {
            "list" => {}
            "revoke" => {
                if value.is_empty() {
                    bail!("Missing credential ID");
                }
                policy::revoke(&path(), &ctx, Some(value))?;
            }
            "revoke_all" => policy::revoke(&path(), &ctx, None)?,
            _ => bail!("Unknown trusted access management command"),
        }
        let devices: Vec<_> = policy::list(&path(),&ctx,now())?.into_iter().map(|record| {
            let public=STANDARD.decode(&record.controller).context("Invalid stored controller identity")?;
            Ok(serde_json::json!({"id":record.id,"name":record.name,"fingerprint":policy::fingerprint(&public),"expiresAt":record.expires_at_ms}))
        }).collect::<ResultType<_>>()?;
        Ok(
            serde_json::json!({"ok":true,"enabled":ctx.enabled && !ctx.two_factor,"devices":devices}),
        )
    })();
    match result { Ok(value)=>value.to_string(), Err(error)=>serde_json::json!({"ok":false,"enabled":enabled(),"devices":[],"error":error.to_string()}).to_string() }
}
