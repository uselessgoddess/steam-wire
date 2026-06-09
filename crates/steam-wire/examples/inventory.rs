//! Dump a Counter-Strike 2 / CS:GO inventory from the game-coordinator welcome.
//!
//! Run with `--features steam-wire-proto/csgo`:
//!
//! ```sh
//! cargo run --example inventory --features steam-wire-proto/csgo -- <account> <password>
//! ```

use std::collections::BTreeMap;
use std::env::args;
use std::error::Error;
use std::io::Cursor;

use steam_wire::auth::{
    AuthConfirmationHandler, ConsoleAuthConfirmationHandler, DeviceConfirmationHandler,
    FileGuardDataStore,
};
use steam_wire::{Connection, ServerList};
use steam_wire_proto::RpcMessage;
use steam_wire_proto::csgo::{CMsgClientHello, CsoEconItem, GCHandshake};

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

    let (_game_coordinator, welcome) = connection
        .game_coordinator(&GCHandshake {
            hello: CMsgClientHello {
                version: Some(2_000_651),
                client_session_need: Some(0),
                client_launcher: Some(0),
                steam_launcher: Some(0),
                ..Default::default()
            },
        })
        .await?;

    let mut inventory = BTreeMap::new();

    for soc in &welcome.outofdate_subscribed_caches {
        for kind in &soc.objects {
            if let Some(1) = kind.type_id {
                for data in &kind.object_data {
                    let item = CsoEconItem::parse(&mut Cursor::new(data))?;
                    inventory.insert(item.id.unwrap_or_default(), item);
                }
            }
        }
    }

    println!("inventory = {inventory:#?}");

    Ok(())
}
