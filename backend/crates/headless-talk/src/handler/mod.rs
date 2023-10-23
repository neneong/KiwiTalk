pub mod error;

use diesel::{BoolExpressionMethods, ExpressionMethods, RunQueryDsl};
use talk_loco_client::talk::stream::{
    command::{DecunRead, Kickout, Msg},
    StreamCommand,
};

use crate::{
    database::{model::chat::ChatRow, schema, DatabasePool},
    event::{channel::ChannelEvent, ClientEvent},
};

use self::error::HandlerError;

type HandlerResult = Result<Option<ClientEvent>, HandlerError>;

#[derive(Debug, Clone)]
pub struct SessionHandler {
    pool: DatabasePool,
}

impl SessionHandler {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }

    pub async fn handle(&self, command: StreamCommand) -> HandlerResult {
        match command {
            StreamCommand::Kickout(kickout) => self.on_kickout(kickout).await,
            StreamCommand::SwitchServer => self.on_switch_server().await,
            StreamCommand::Chat(msg) => self.on_chat(msg).await,
            StreamCommand::ChatRead(read) => self.on_chat_read(read).await,

            _ => Ok(None),
        }
    }

    async fn on_kickout(&self, kickout: Kickout) -> HandlerResult {
        Ok(Some(ClientEvent::Kickout(kickout.reason)))
    }

    async fn on_switch_server(&self) -> HandlerResult {
        Ok(Some(ClientEvent::SwitchServer))
    }

    async fn on_chat(&self, msg: Msg) -> HandlerResult {
        self.pool
            .spawn({
                let chatlog = msg.chatlog.clone();

                move |mut conn| {
                    diesel::insert_into(schema::chat::table)
                        .values(ChatRow::from_chatlog(&chatlog, None))
                        .execute(&mut conn)?;

                    Ok(())
                }
            })
            .await?;

        Ok(Some(ClientEvent::Channel {
            id: msg.chat_id,

            event: ChannelEvent::Chat {
                link_id: msg.link_id,

                user_nickname: msg.author_nickname,
                chat: msg.chatlog,
            },
        }))
    }

    async fn on_chat_read(&self, read: DecunRead) -> HandlerResult {
        self.pool
            .spawn({
                let DecunRead {
                    chat_id: channel_id,
                    user_id,
                    watermark,
                } = read.clone();

                move |mut conn| {
                    use schema::user_profile;

                    diesel::update(user_profile::table)
                        .filter(
                            user_profile::channel_id
                                .eq(channel_id)
                                .and(user_profile::id.eq(user_id)),
                        )
                        .set(user_profile::watermark.eq(watermark))
                        .execute(&mut conn)?;
                    Ok(())
                }
            })
            .await?;

        Ok(Some(ClientEvent::Channel {
            id: read.chat_id,

            event: ChannelEvent::ChatRead {
                user_id: read.user_id,
                log_id: read.watermark,
            },
        }))
    }
}
