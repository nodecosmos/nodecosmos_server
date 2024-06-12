use charybdis::types::{Set, Uuid};

use crate::models::branch::{AuthBranch, Branch, BranchStatus};
use crate::models::comment::Comment;
use crate::models::comment_thread::CommentThread;
use crate::models::contribution_request::{ContributionRequest, ContributionRequestStatus};
use crate::models::invitation::Invitation;
use crate::models::user::User;

/// AuthorizationFields for nodes is implemented with the `NodeAuthorization` derive macro.
pub trait AuthorizationFields {
    fn owner_id(&self) -> Option<Uuid>;

    fn editor_ids(&self) -> &Option<Set<Uuid>>;

    fn viewer_ids(&self) -> &Option<Set<Uuid>>;

    fn is_frozen(&self) -> bool;

    fn is_public(&self) -> bool;
}

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
        return self.node.as_ref().and_then(|node| Some(node.owner_id));
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
