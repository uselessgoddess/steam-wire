mod confirmation;
mod guard_data;

use std::time::Duration;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
pub use confirmation::*;
pub use guard_data::*;
use bytes::Bytes;
use rsa::{BoxedUint, RsaPublicKey};
use steam_wire_crypto::encrypt_with_key_pkcs1;
use steam_wire_proto_steam::{
    CAuthenticationAllowedConfirmation, CAuthenticationBeginAuthSessionViaCredentialsRequest,
    CAuthenticationBeginAuthSessionViaCredentialsResponse, CAuthenticationDeviceDetails,
    CAuthenticationGetPasswordRsaPublicKeyRequest, CAuthenticationPollAuthSessionStatusRequest,
    CAuthenticationPollAuthSessionStatusResponse,
    CAuthenticationUpdateAuthSessionWithSteamGuardCodeRequest, EAuthSessionGuardType,
    EAuthTokenPlatformType, ESessionPersistence,
};
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, info, instrument};

use crate::connection::raw::RawConnection;
use crate::connection::unauthenticated::service_method_un_authenticated;
use crate::message::{
    MalformedBody, MessageBodyError, NetMessage, ParseBigIntError, ServiceMethodMessage,
};
use crate::net::NetworkError;
use crate::session::{ConnectionError, LoginError};

pub(crate) async fn begin_password_auth(
    connection: &mut RawConnection,
    account: &str,
    password: &str,
    guard_data: Option<&str>,
) -> Result<StartedAuth, ConnectionError> {
    let (pub_key, timestamp) = get_password_rsa(connection, account.into()).await?;
    let encrypted_password =
        encrypt_with_key_pkcs1(&pub_key, password.as_bytes()).map_err(LoginError::InvalidPubKey)?;
    let encoded_password = BASE64_STANDARD.encode(encrypted_password);
    info!(account, "starting credentials login");
    let req = CAuthenticationBeginAuthSessionViaCredentialsRequest {
        account_name: Some(account.into()),
        encrypted_password: Some(encoded_password),
        encryption_timestamp: Some(timestamp),
        persistence: Some(ESessionPersistence::KESessionPersistencePersistent as i32),

        // todo: platform types
        website_id: Some("Client".into()),
        device_details: Some(CAuthenticationDeviceDetails {
            device_friendly_name: Some("DESKTOP-VENT".into()),
            platform_type: Some(EAuthTokenPlatformType::KEAuthTokenPlatformTypeSteamClient as i32),
            os_type: Some(1),
            ..CAuthenticationDeviceDetails::default()
        }),
        guard_data: guard_data.map(String::from),
        ..CAuthenticationBeginAuthSessionViaCredentialsRequest::default()
    };
    let res = service_method_un_authenticated(connection, req).await?;
    Ok(StartedAuth::Credentials(res))
}

pub(crate) enum StartedAuth {
    Credentials(CAuthenticationBeginAuthSessionViaCredentialsResponse),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfirmationError {
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error("Aborted")]
    Aborted,
}

impl StartedAuth {
    fn raw_confirmations(&self) -> &[CAuthenticationAllowedConfirmation] {
        match self {
            StartedAuth::Credentials(res) => res.allowed_confirmations.as_slice(),
        }
    }

    pub fn allowed_confirmations(&self) -> Vec<ConfirmationMethod> {
        self.raw_confirmations().iter().cloned().map(ConfirmationMethod::from).collect()
    }

    #[allow(dead_code)]
    pub fn action_required(&self) -> bool {
        self.raw_confirmations().iter().any(|method| {
            method.confirmation_type.unwrap_or_default()
                != EAuthSessionGuardType::KEAuthSessionGuardTypeNone as i32
        })
    }

    fn client_id(&self) -> u64 {
        match self {
            StartedAuth::Credentials(res) => res.client_id.unwrap_or(0),
        }
    }

    pub fn steam_id(&self) -> u64 {
        match self {
            StartedAuth::Credentials(res) => res.steamid.unwrap_or(0),
        }
    }

    fn request_id(&self) -> Vec<u8> {
        match self {
            StartedAuth::Credentials(res) => res.request_id.as_deref().unwrap_or_default().to_vec(),
        }
    }

    fn interval(&self) -> f32 {
        match self {
            StartedAuth::Credentials(res) => res.interval.unwrap_or(0.0),
        }
    }

    pub fn poll(&self) -> PendingAuth {
        PendingAuth {
            interval: self.interval(),
            client_id: self.client_id(),
            request_id: self.request_id(),
        }
    }

    pub async fn submit_confirmation(
        &self,
        connection: &RawConnection,
        confirmation: ConfirmationAction,
    ) -> Result<(), ConfirmationError> {
        match confirmation {
            ConfirmationAction::GuardToken(token, ty) => {
                let req = CAuthenticationUpdateAuthSessionWithSteamGuardCodeRequest {
                    client_id: Some(self.client_id()),
                    steamid: Some(self.steam_id()),
                    code: Some(token.0),
                    code_type: Some(EAuthSessionGuardType::from(ty) as i32),
                };
                let _ = service_method_un_authenticated(connection, req).await?;
            }
            ConfirmationAction::None => {}
            ConfirmationAction::Abort => return Err(ConfirmationError::Aborted),
        };
        Ok(())
    }
}

/// The token to send to steam to confirm the login
#[derive(Debug)]
pub struct SteamGuardToken(String);

pub(crate) struct PendingAuth {
    client_id: u64,
    request_id: Vec<u8>,
    interval: f32,
}

impl PendingAuth {
    pub(crate) async fn wait_for_tokens(
        self,
        connection: &RawConnection,
    ) -> Result<Tokens, NetworkError> {
        loop {
            let mut response = poll_until_info(
                connection,
                self.client_id,
                &self.request_id,
                Duration::from_secs_f32(self.interval),
            )
            .await?;
            if response.access_token.is_some() {
                return Ok(Tokens {
                    access_token: Token(response.access_token.take().unwrap_or_default()),
                    refresh_token: Token(response.refresh_token.take().unwrap_or_default()),
                    new_guard_data: response.new_guard_data,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Token(String);

impl AsRef<str> for Token {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Tokens {
    #[allow(dead_code)]
    pub access_token: Token,
    pub refresh_token: Token,
    pub new_guard_data: Option<String>,
}

async fn poll_until_info(
    connection: &RawConnection,
    client_id: u64,
    request_id: &[u8],
    interval: Duration,
) -> Result<CAuthenticationPollAuthSessionStatusResponse, NetworkError> {
    loop {
        let req = CAuthenticationPollAuthSessionStatusRequest {
            client_id: Some(client_id),
            request_id: Some(Bytes::copy_from_slice(request_id)),
            ..CAuthenticationPollAuthSessionStatusRequest::default()
        };

        let resp = service_method_un_authenticated(connection, req).await?;
        let has_data = resp.access_token.is_some()
            || resp.account_name.is_some()
            || resp.agreement_session_url.is_some()
            || resp.had_remote_interaction.is_some()
            || resp.new_challenge_url.is_some()
            || resp.new_client_id.is_some()
            || resp.new_guard_data.is_some()
            || resp.refresh_token.is_some();

        if has_data {
            return Ok(resp);
        }

        sleep(interval).await;
    }
}

fn hex_to_boxed_uint(hex_str: &str) -> Option<BoxedUint> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);

    if hex_str.is_empty() {
        return None;
    }

    let bytes = hex::decode(hex_str).ok()?;
    let precision = (bytes.len() as u32 * 8).next_multiple_of(64);

    BoxedUint::from_be_slice(&bytes, precision).ok()
}

fn malformed(error: impl Into<MessageBodyError>) -> MalformedBody {
    MalformedBody::new(
        ServiceMethodMessage::<CAuthenticationGetPasswordRsaPublicKeyRequest>::KIND,
        error,
    )
}

#[instrument(skip(connection))]
async fn get_password_rsa(
    connection: &mut RawConnection,
    account: String,
) -> Result<(RsaPublicKey, u64), NetworkError> {
    debug!("getting password rsa");
    let req = CAuthenticationGetPasswordRsaPublicKeyRequest { account_name: Some(account) };
    let response = service_method_un_authenticated(connection, req).await?;

    let n_hex = response.publickey_mod.as_deref().unwrap_or_default();
    let e_hex = response.publickey_exp.as_deref().unwrap_or_default();

    let n = hex_to_boxed_uint(n_hex).ok_or(ParseBigIntError).map_err(malformed)?;
    let e = hex_to_boxed_uint(e_hex).ok_or(ParseBigIntError).map_err(malformed)?;
    let key = RsaPublicKey::new(n, e).map_err(malformed)?;

    Ok((key, response.timestamp.unwrap_or_default()))
}
