use crate::models::{Kit, KitMembership, User};
use serde::Serialize;

pub trait Permission {
    type Actor;
    type Object;

    fn permitted(self, actor: &Self::Actor, object: &Self::Object) -> bool;
}

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum KitAction {
    View,
    SubscribeRealTimeMeasurements,
    ResetPassword,
    EditDetails,
    EditConfiguration,
    EditMembers,
    SetSuperMember,
    RpcVersion,
    RpcUptime,
}

pub enum KitUser {
    Anonymous,
    User(User),
    UserWithMembership(User, KitMembership),
}

impl Permission for KitAction {
    type Actor = KitUser;
    type Object = Kit;

    fn permitted(self, user: &KitUser, kit: &Kit) -> bool {
        use KitAction::*;
        use KitUser::*;
        match user {
            Anonymous | User(..) => match self {
                View | SubscribeRealTimeMeasurements => kit.privacy_show_on_map,
                _ => false,
            },
            UserWithMembership(_user, membership) => match self {
                View | SubscribeRealTimeMeasurements => true,
                EditDetails | EditConfiguration => membership.access_configure,
                ResetPassword | EditMembers | SetSuperMember => membership.access_super,
                RpcVersion | RpcUptime => membership.access_super,
            },
        }
    }
}

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum UserAction {
    View,
}

impl Permission for UserAction {
    type Actor = Option<User>;
    type Object = User;

    fn permitted(self, _acting_user: &Option<User>, _object_user: &User) -> bool {
        use UserAction::*;
        match self {
            View => true,
        }
    }
}
