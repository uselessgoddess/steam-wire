use std::env::args;
use std::error::Error;
use std::io::stdin;
use std::str::FromStr;

use steam_wire::auth::{
    AuthConfirmationHandler, ConsoleAuthConfirmationHandler, DeviceConfirmationHandler,
    FileGuardDataStore,
};
use steam_wire::{Connection, ConnectionTrait, ServerList};
use steam_wire_proto::{
    CFriendMessagesIncomingMessageNotification, CFriendMessagesSendMessageRequest,
    CMsgClientChangeStatus, EPersonaStateFlag,
};
use steamid_ng::SteamID;
use tokio::spawn;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let mut args = args().skip(1);
    let account = args.next().expect("no account");
    let password = args.next().expect("no password");
    let target_steam_id = SteamID::from_str(args.next().expect("no target steam id").as_str())
        .expect("invalid steam id");

    let server_list = ServerList::discover().await?;
    let connection = Connection::login(
        &server_list,
        &account,
        &password,
        FileGuardDataStore::user_cache(),
        ConsoleAuthConfirmationHandler::default().or(DeviceConfirmationHandler),
    )
    .await?;

    connection
        .send(CMsgClientChangeStatus {
            persona_state: Some(1),
            persona_state_flags: Some(
                EPersonaStateFlag::KEPersonaStateFlagClientTypeMobile as u32,
            ),
            ..Default::default()
        })
        .await?;

    let mut incoming_messages =
        connection.on_notification::<CFriendMessagesIncomingMessageNotification>();
    spawn(async move {
        while let Some(Ok(incoming)) = incoming_messages.next().await {
            println!(
                "{}: {}",
                incoming.steamid_friend.unwrap_or(0),
                incoming.message.as_deref().unwrap_or_default()
            );
        }
    });
    let mut read_buff = String::with_capacity(32);
    loop {
        read_buff.clear();
        stdin().read_line(&mut read_buff).expect("stdin error");
        let input = read_buff.trim();
        if !input.is_empty() {
            let req = CFriendMessagesSendMessageRequest {
                steamid: Some(target_steam_id.into()),
                message: Some(input.into()),
                chat_entry_type: Some(1),
                ..CFriendMessagesSendMessageRequest::default()
            };
            connection.service_method(req).await?;
        }
    }
}
