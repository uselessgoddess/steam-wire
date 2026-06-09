use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::Value;

pub fn exp_claim_unix_secs(jwt: &str) -> Option<u64> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload_b64.as_bytes()).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    let exp = v.get("exp")?;
    exp.as_u64()
        .or_else(|| exp.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| exp.as_f64().map(|f| f as u64))
}

pub fn is_expired_now(jwt: &str) -> bool {
    let Some(exp) = exp_claim_unix_secs(jwt) else {
        return false;
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    now >= exp
}
