use crate::models::{Kit, KitMembership, User};
use serde::Serialize;

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum KitAction {
    View,
    SubscribeRealTimeMeasurements,
    EditDetails,
    EditConfiguration,
    EditMembers,
    SetSuperMember,
}

impl KitAction {
    pub fn permission(
        self,
        _user: &Option<User>,
        kit_membership: &Option<KitMembership>,
        kit: &Kit,
    ) -> bool {
        use KitAction::*;
        match self {
            View | SubscribeRealTimeMeasurements => {
                kit.privacy_public_dashboard || kit_membership.is_some()
            }
            EditDetails | EditConfiguration => kit_membership
                .as_ref()
                .map(|m| m.access_configure)
                .unwrap_or(false),
            EditMembers | SetSuperMember => kit_membership
                .as_ref()
                .map(|m| m.access_super)
                .unwrap_or(false),
        }
    }
}
