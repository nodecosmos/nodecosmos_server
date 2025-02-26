use charybdis::types::{Set, Uuid};

use crate::models::branch::{AuthBranch, Branch, BranchStatus};
use crate::models::comment::Comment;
use crate::models::comment_thread::CommentThread;
use crate::models::contribution_request::{ContributionRequest, ContributionRequestStatus};
use crate::models::invitation::Invitation;
use crate::models::node::{AuthNode, BaseNode, Node, UpdateTitleNode};
use crate::models::traits::Branchable;
use crate::models::user::User;

pub trait AuthorizationFields {
    fn owner_id(&self) -> Option<Uuid>;

    fn editor_ids(&self) -> &Option<Set<Uuid>>;

    fn viewer_ids(&self) -> &Option<Set<Uuid>>;

    fn is_frozen(&self) -> bool;

    fn is_public(&self) -> bool;

    fn is_subscription_active(&self) -> bool {
        true
    }
}

macro_rules! impl_auth_fields_for_node_type {
    ($struct_name:ident) => {
        impl AuthorizationFields for $struct_name {
            fn owner_id(&self) -> Option<Uuid> {
                if self.is_original() {
                    return Some(self.owner_id);
                }

                match &self.auth_branch {
                    Some(branch) => Some(branch.owner_id),
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        None
                    }
                }
            }

            fn editor_ids(&self) -> &Option<Set<Uuid>> {
                if self.is_original() {
                    return &self.editor_ids;
                }

                match &self.auth_branch {
                    Some(branch) => &branch.editor_ids,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        &None
                    }
                }
            }

            fn viewer_ids(&self) -> &Option<Set<Uuid>> {
                if self.is_original() {
                    return &self.viewer_ids;
                }

                match &self.auth_branch {
                    Some(branch) => &branch.viewer_ids,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        &None
                    }
                }
            }

            fn is_frozen(&self) -> bool {
                if self.is_branch() {
                    return match &self.auth_branch {
                        Some(branch) => branch.is_frozen(),
                        None => {
                            log::error!("Branched node {} has no branch!", self.id);

                            false
                        }
                    };
                }

                false
            }

            fn is_public(&self) -> bool {
                if self.is_original() {
                    return self.is_public;
                }

                match &self.auth_branch {
                    Some(branch) => branch.is_public,
                    None => {
                        log::error!("Branched node {} has no branch!", self.id);

                        false
                    }
                }
            }

            fn is_subscription_active(&self) -> bool {
                if self.is_public() {
                    return true;
                }

                self.is_subscription_active
                    .map_or(false, |is_sub_active| is_sub_active)
            }
        }
    };
}

impl_auth_fields_for_node_type!(Node);
impl_auth_fields_for_node_type!(AuthNode);
impl_auth_fields_for_node_type!(BaseNode);
impl_auth_fields_for_node_type!(UpdateTitleNode);

impl AuthorizationFields for Branch {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.owner_id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        &self.editor_ids
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        &self.viewer_ids
    }

    fn is_frozen(&self) -> bool {
        self.status == Some(BranchStatus::Merged.to_string())
            || self.status == Some(BranchStatus::RecoveryFailed.to_string())
            || self.status == Some(BranchStatus::Closed.to_string())
    }

    fn is_public(&self) -> bool {
        self.is_public
    }
}

impl AuthorizationFields for AuthBranch {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.owner_id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        &self.editor_ids
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        &self.viewer_ids
    }

    fn is_frozen(&self) -> bool {
        self.status == Some(BranchStatus::Merged.to_string())
            || self.status == Some(BranchStatus::RecoveryFailed.to_string())
            || self.status == Some(BranchStatus::Closed.to_string())
    }

    fn is_public(&self) -> bool {
        self.is_public
    }
}

impl AuthorizationFields for ContributionRequest {
    fn owner_id(&self) -> Option<Uuid> {
        self.branch.as_ref().map(|branch| branch.owner_id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        self.branch.as_ref().map_or(&None, |branch| &branch.editor_ids)
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        self.branch.as_ref().map_or(&None, |branch| &branch.viewer_ids)
    }

    // ATM this works fine because we only find CR before merging it, while allowing CR description
    // to be changed after merging. However, this is accidental and it works because
    // we don't query CR before other update operations.
    fn is_frozen(&self) -> bool {
        self.status == Some(ContributionRequestStatus::Merged.to_string())
            || self.status == Some(ContributionRequestStatus::Closed.to_string())
    }

    fn is_public(&self) -> bool {
        self.branch.as_ref().map_or(false, |branch| branch.is_public)
    }
}

impl AuthorizationFields for User {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn is_frozen(&self) -> bool {
        false
    }

    fn is_public(&self) -> bool {
        false
    }
}

impl AuthorizationFields for CommentThread {
    fn owner_id(&self) -> Option<Uuid> {
        self.author_id
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn is_frozen(&self) -> bool {
        false
    }

    fn is_public(&self) -> bool {
        false
    }
}

impl AuthorizationFields for Comment {
    fn owner_id(&self) -> Option<Uuid> {
        Some(self.author_id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        &None
    }

    fn is_frozen(&self) -> bool {
        false
    }

    fn is_public(&self) -> bool {
        false
    }
}

impl AuthorizationFields for Invitation {
    fn owner_id(&self) -> Option<Uuid> {
        self.node.as_ref().map(|node| node.owner_id)
    }

    fn editor_ids(&self) -> &Option<Set<Uuid>> {
        if let Some(node) = &self.node {
            return &node.editor_ids;
        }

        &None
    }

    fn viewer_ids(&self) -> &Option<Set<Uuid>> {
        if let Some(node) = &self.node {
            return &node.viewer_ids;
        }

        &None
    }

    fn is_frozen(&self) -> bool {
        false
    }

    fn is_public(&self) -> bool {
        false
    }
}
