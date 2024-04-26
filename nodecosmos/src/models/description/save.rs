use crate::api::data::RequestData;
use crate::errors::NodecosmosError;
use crate::models::branch::update::BranchUpdate;
use crate::models::branch::Branch;
use crate::models::description::Description;
use crate::models::flow::Flow;
use crate::models::flow_step::FlowStep;
use crate::models::io::Io;
use crate::models::node::Node;
use crate::models::traits::{
    Branchable, ElasticDocument, FindOrInsertBranched, ObjectType, UpdateNodeDescriptionElasticIdx,
};

impl Description {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branched() {
            match ObjectType::from(self.object_type.parse()?) {
                ObjectType::Node => {
                    Node::find_or_insert_branched(data, self.object_id, self.branch_id, self.object_id).await?;

                    Branch::update(&data, self.branch_id, BranchUpdate::EditNodeDescription(self.object_id)).await?;
                }
                ObjectType::Flow => {
                    Flow::find_or_insert_branched(data, self.node_id, self.branch_id, self.object_id).await?;

                    Branch::update(&data, self.branch_id, BranchUpdate::EditFlowDescription(self.object_id)).await?;
                }
                ObjectType::FlowStep => {
                    FlowStep::find_or_insert_branched(data, self.node_id, self.branch_id, self.object_id).await?;

                    Branch::update(
                        &data,
                        self.branch_id,
                        BranchUpdate::EditFlowStepDescription(self.object_id),
                    )
                    .await?;
                }
                ObjectType::Io => {
                    Io::find_or_insert_branched_main(data, self.node_id, self.branch_id, self.object_id).await?;

                    Branch::update(&data, self.branch_id, BranchUpdate::EditIoDescription(self.object_id)).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn update_elastic_index(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() {
            match ObjectType::from(self.object_type.parse()?) {
                ObjectType::Node => {
                    UpdateNodeDescriptionElasticIdx {
                        id: self.object_id,
                        short_description: self.short_description.clone().unwrap_or_default(),
                        description: self.html.clone().unwrap_or_default(),
                    }
                    .update_elastic_document(data.elastic_client())
                    .await;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
