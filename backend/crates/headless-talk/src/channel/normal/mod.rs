pub mod user;

use diesel::{
    BoolExpressionMethods, ExpressionMethods, JoinOnDsl, OptionalExtension, QueryDsl, RunQueryDsl,
    SelectableHelper,
};
use talk_loco_client::talk::channel::ChannelMetaType;

use crate::{
    database::{
        model::{
            channel::ChannelListRow,
            user::{normal::NormalChannelUserModel, UserProfileModel},
        },
        schema::{channel_meta, normal_channel_user, user_profile},
        DatabasePool, PoolTaskError,
    },
    user::DisplayUser,
    HeadlessTalk,
};

use self::user::NormalChannelUser;

use super::ListChannelProfile;

#[derive(Debug, Clone)]
pub struct NormalChannel<'a> {
    id: i64,
    client: &'a HeadlessTalk,
}

impl<'a> NormalChannel<'a> {
    pub(crate) const fn new(id: i64, client: &'a HeadlessTalk) -> Self {
        Self { id, client }
    }

    pub const fn id(&self) -> i64 {
        self.id
    }

    pub const fn client(&self) -> &'a HeadlessTalk {
        self.client
    }

    pub async fn users(&self) -> Result<Vec<NormalChannelUser>, PoolTaskError> {
        let users = self
            .client
            .pool
            .spawn({
                let id = self.id;

                move |conn| {
                    let users: Vec<NormalChannelUser> = user_profile::table
                        .inner_join(
                            normal_channel_user::table.on(normal_channel_user::channel_id
                                .eq(user_profile::channel_id)
                                .and(normal_channel_user::id.eq(user_profile::id))),
                        )
                        .filter(user_profile::channel_id.eq(id))
                        .select((
                            UserProfileModel::as_select(),
                            NormalChannelUserModel::as_select(),
                        ))
                        .load_iter::<(UserProfileModel, NormalChannelUserModel), _>(conn)?
                        .map(|res| {
                            res.map(|(profile, normal)| {
                                NormalChannelUser::from_models(profile, normal)
                            })
                        })
                        .collect::<Result<_, _>>()?;

                    Ok(users)
                }
            })
            .await?;

        Ok(users)
    }
}

pub(super) async fn load_list_profile(
    pool: &DatabasePool,
    display_users: &[DisplayUser],
    row: &ChannelListRow,
) -> Result<ListChannelProfile, PoolTaskError> {
    let id = row.id;

    let (name, image_url) = pool
        .spawn(move |conn| {
            let name: Option<String> = channel_meta::table
                .filter(
                    channel_meta::channel_id
                        .eq(id)
                        .and(channel_meta::type_.eq(ChannelMetaType::Title as i32)),
                )
                .select(channel_meta::content)
                .first(conn)
                .optional()?;

            let image_url: Option<String> = channel_meta::table
                .filter(
                    channel_meta::channel_id
                        .eq(id)
                        .and(channel_meta::type_.eq(ChannelMetaType::Profile as i32)),
                )
                .select(channel_meta::content)
                .first(conn)
                .optional()?;

            Ok((name, image_url))
        })
        .await?;

    let name = name.unwrap_or_else(|| {
        display_users
            .iter()
            .map(|user| user.profile.nickname.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
    });

    Ok(ListChannelProfile { name, image_url })
}
