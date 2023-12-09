use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::contribution_request::status::ContributionRequestStatus;
use crate::models::contribution_request::ContributionRequest;
use charybdis::operations::UpdateWithExtCallbacks;

impl ContributionRequest {
    pub async fn publish(&mut self, data: &RequestData) -> Result<(), NodecosmosError> {
        self.update_status(data, ContributionRequestStatus::Published).await?;

        Ok(())
    }

    async fn update_status(
        &mut self,
        data: &RequestData,
        status: ContributionRequestStatus,
    ) -> Result<(), NodecosmosError> {
        self.status = Some(status.to_string());

        self.update_cb(data.db_session(), data).await?;

        Ok(())
    }
}
