use serde::Serialize;
use crate::models::{Kit, KitMembership, User};

#[derive(Serialize, Copy, Clone, Debug, EnumIter)]
pub enum KitAction {
    view,
    subscribeRealTimeMeasurements,
    editDetails,
    editConfiguration,
    editMembers,
    setSuperMember,
}

impl KitAction {
    pub fn permission(
        self,
        user: &Option<User>,
        kit_membership: &Option<KitMembership>,
        kit: &Kit,
    ) -> bool {
        use KitAction::*;
        match self {
            view | subscribeRealTimeMeasurements => {
                kit.privacy_public_dashboard || kit_membership.is_some()
            }
            editDetails | editConfiguration => {
                kit_membership.as_ref().map(|m| m.access_configure).unwrap_or(false)
            }
            editMembers | setSuperMember => kit_membership.as_ref().map(|m| m.access_super).unwrap_or(false),
        }
    }
}
