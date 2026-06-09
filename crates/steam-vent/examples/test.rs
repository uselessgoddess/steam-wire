use std::error::Error;

use steam_vent::{Connection, ConnectionTrait, ServerList};
use steam_vent_proto::steammessages_contentsystem_steamclient::CContentServerDirectory_GetServersForSteamPipe_Request;

/// Lists Steam content (CDN) servers over an anonymous connection.
///
/// This example used to call `IGameServers.GetServerList` (a TF2 server-browser
/// query), but Steam removed that RPC from its protobufs. `ContentServerDirectory
/// .GetServersForSteamPipe` is the modern, anonymous-accessible equivalent that
/// returns a server list to iterate, so this stays a working demonstration of
/// [`ConnectionTrait::service_method`].
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let server_list = ServerList::discover().await?;
    let connection = Connection::anonymous(&server_list).await?;

    println!("requesting content servers");

    let mut req = CContentServerDirectory_GetServersForSteamPipe_Request::new();
    req.set_max_servers(16);
    let response = connection.service_method(req).await?;
    for server in response.servers {
        println!(
            "{} {} (load {})",
            server.type_(),
            server.host(),
            server.load(),
        );
    }

    Ok(())
}
