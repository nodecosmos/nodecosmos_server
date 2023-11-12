use crate::errors::NodecosmosError;
use crate::models::workflow::Workflow;
use charybdis::types::Uuid;
use scylla::CachingSession;

impl Workflow {
    pub async fn pull_initial_input_id(&mut self, session: &CachingSession, id: Uuid) -> Result<(), NodecosmosError> {
        self.pull_from_initial_input_ids(session, &vec![id]).await?;

        Ok(())
    }
}
