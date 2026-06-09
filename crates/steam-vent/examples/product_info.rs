use std::error::Error;

use steam_vent::{Connection, ConnectionTrait, ServerList};
use steam_vent_proto::{
    CMsgClientPicsProductInfoRequest, CMsgClientPicsProductInfoResponse,
    c_msg_client_pics_product_info_request,
};
use vdf_reader::entry::Table;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let server_list = ServerList::discover().await?;
    let connection = Connection::anonymous(&server_list).await?;

    let msg = CMsgClientPicsProductInfoRequest {
        apps: vec![c_msg_client_pics_product_info_request::AppInfo {
            appid: Some(440),
            only_public_obsolete: Some(true),
            ..Default::default()
        }],
        meta_data_only: Some(false),
        single_response: Some(true),
        ..Default::default()
    };

    let response: CMsgClientPicsProductInfoResponse = connection.job(msg).await?;
    let buffer = response.apps[0].buffer.as_deref().unwrap_or_default();
    let vdf = String::from_utf8(buffer.into())?;
    let vdf = vdf.trim().trim_matches('\0');
    let parsed: Table = vdf_reader::from_str(vdf)?;
    dbg!(parsed);

    Ok(())
}
