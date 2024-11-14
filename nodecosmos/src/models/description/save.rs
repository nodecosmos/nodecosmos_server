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
    Branchable, ElasticDocument, FindOrInsertBranched, ModelBranchParams, ObjectType, UpdateNodeDescriptionElasticIdx,
};

impl Description {
    pub async fn update_branch(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_branch() {
            let params = ModelBranchParams {
                original_id: self.original_id(),
                branch_id: self.branch_id,
                id: self.object_id,
            };

            match self.object_type.parse::<ObjectType>()? {
                ObjectType::Node => {
                    Node::find_or_insert_branched(data, params).await?;

                    Branch::update(
                        data.db_session(),
                        self.branch_id,
                        BranchUpdate::EditNodeDescription(self.object_id),
                    )
                    .await?;
                }
                ObjectType::Flow => {
                    Flow::find_or_insert_branched(data, params).await?;

                    Branch::update(
                        data.db_session(),
                        self.branch_id,
                        BranchUpdate::EditFlowDescription(self.object_id),
                    )
                    .await?;
                }
                ObjectType::FlowStep => {
                    FlowStep::find_or_insert_branched(data, params).await?;

                    Branch::update(
                        data.db_session(),
                        self.branch_id,
                        BranchUpdate::EditFlowStepDescription(self.object_id),
                    )
                    .await?;
                }
                ObjectType::Io => {
                    Io::find_or_insert_branched_main(data, params.original_id, params.branch_id, params.id).await?;

                    Branch::update(
                        data.db_session(),
                        self.branch_id,
                        BranchUpdate::EditIoDescription(self.object_id),
                    )
                    .await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn update_elastic_index(&self, data: &RequestData) -> Result<(), NodecosmosError> {
        if self.is_original() && self.object_type.parse::<ObjectType>()? == ObjectType::Node {
            UpdateNodeDescriptionElasticIdx {
                id: self.object_id,
                short_description: self.short_description.clone().unwrap_or_default(),
                description: self.html.clone().unwrap_or_default(),
            }
            .update_elastic_document(data.elastic_client())
            .await;
        }

        Ok(())
    }
}
