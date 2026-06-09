//! Dump the item definition indices in a Team Fortress 2 backpack.
//!
//! Run with `--features steam-wire-proto/tf2`:
//!
//! ```sh
//! cargo run --example backpack --features steam-wire-proto/tf2 -- <account> <password>
//! ```

use std::env::args;
use std::error::Error;
use std::io::Cursor;

use steam_wire::auth::{
    AuthConfirmationHandler, ConsoleAuthConfirmationHandler, DeviceConfirmationHandler,
    FileGuardDataStore,
};
use steam_wire::{Connection, ConnectionTrait, ServerList};
use steam_wire_proto::RpcMessage;
use steam_wire_proto::tf2::{
    CMsgSoCacheSubscribed, CMsgSoCacheSubscriptionRefresh, CsoEconItem, GCHandshake,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let mut args = args().skip(1);
    let account = args.next().expect("no account");
    let password = args.next().expect("no password");

    let server_list = ServerList::discover().await?;
    let connection = Connection::login(
        &server_list,
        &account,
        &password,
        FileGuardDataStore::user_cache(),
        ConsoleAuthConfirmationHandler::default().or(DeviceConfirmationHandler),
    )
    .await?;

    println!("starting game coordinator");

    let (game_coordinator, _welcome) =
        connection.game_coordinator(&GCHandshake::default()).await?;

    println!("requesting backpack");

    let cache_future = game_coordinator.one::<CMsgSoCacheSubscribed>();
    game_coordinator
        .send(CMsgSoCacheSubscriptionRefresh {
            owner: Some(connection.steam_id().into()),
            ..Default::default()
        })
        .await?;
    let cache = cache_future.await?;
    for object in &cache.objects {
        if object.type_id == Some(1) {
            for item_data in &object.object_data {
                if let Ok(item) = CsoEconItem::parse(&mut Cursor::new(item_data)) {
                    // this indexes into the item schema
                    println!("{}", item.def_index.unwrap_or_default());
                }
            }
        }
    }

    Ok(())
}
