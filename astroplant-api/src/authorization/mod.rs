use crate::models::{Kit, KitMembership, User};
use serde::Serialize;

pub trait Permission {
    type Actor;
    type Object;

    fn permitted(self, actor: &Self::Actor, object: &Self::Object) -> bool;
}

// Perhaps an "evidence" based permission system could work (with fewer chances for bugs).
// Helps prevent boolean blindness, but boilerplate-y to implement.
//
// ```
// pub trait Permission_<Subject, Object> {
//     type Evidence<'obj>;
//
//     /// If the subject is permitted to act on the object, a value of a type enabling that
//     /// behavior is returned.
//     fn permitted<'obj>(
//         self,
//         subject: &Subject,
//         object: &'obj mut Object,
//     ) -> Result<Self::Evidence<'obj>, ()>;
// }
//
// mod kit_actions {
//     pub(crate) struct ResetPassword;
// }
//
// impl Permission_<KitUser, Kit> for kit_actions::ResetPassword {
//     type Evidence<'obj> = KitResetPasswordHandle<'obj>;
//     // ...
// }
// ```

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum KitAction {
    View,
    SubscribeRealTimeMeasurements,
    Delete,
    DeleteMedia,
    ResetPassword,
    EditDetails,
    EditConfiguration,
    EditMembers,
    EditSuperMembers,
    RpcVersion,
    RpcUptime,
    RpcPeripheralCommand,
    RpcPeripheralCommandLock,
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
                View | SubscribeRealTimeMeasurements => kit.privacy_public_dashboard,
                _ => false,
            },
            UserWithMembership(_user, membership) => match self {
                View | SubscribeRealTimeMeasurements => true,
                EditDetails | EditConfiguration | DeleteMedia => {
                    membership.access_configure || membership.access_super
                }
                Delete | ResetPassword | EditMembers | EditSuperMembers => membership.access_super,
                RpcVersion | RpcUptime | RpcPeripheralCommand | RpcPeripheralCommandLock => {
                    membership.access_super
                }
            },
        }
    }
}

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum UserAction {
    View,
    ListKitMemberships,
    EditDetails,
}

impl Permission for UserAction {
    type Actor = Option<User>;
    type Object = User;

    fn permitted(self, acting_user: &Option<User>, object_user: &User) -> bool {
        use UserAction::*;
        match self {
            View | ListKitMemberships => true,
            EditDetails => acting_user
                .as_ref()
                .map(|acting_user| acting_user == object_user)
                .unwrap_or(false),
        }
    }
}
