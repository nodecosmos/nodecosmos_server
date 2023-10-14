use charybdis::types::Text;
use std::fmt::Display;

pub enum ContributionRequestStatus {
    WorkInProgress,
    // Published,
    // Merged,
    // Closed,
}

impl Display for ContributionRequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContributionRequestStatus::WorkInProgress => write!(f, "WORK_IN_PROGRESS"),
            // ContributionRequestStatus::Published => write!(f, "PUBLISHED"),
            // ContributionRequestStatus::Merged => write!(f, "MERGED"),
            // ContributionRequestStatus::Closed => write!(f, "CLOSED"),
        }
    }
}

pub fn default_status() -> Option<Text> {
    Some(ContributionRequestStatus::WorkInProgress.to_string())
}
