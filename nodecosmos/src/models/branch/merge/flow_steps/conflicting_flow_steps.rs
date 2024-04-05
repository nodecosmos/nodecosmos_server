// use crate::errors::NodecosmosError;
// use crate::models::branch::Branch;
// use crate::models::flow_step::{find_flow_step, FlowStep};
// use crate::models::traits::Branchable;
// use charybdis::types::Uuid;
// use scylla::CachingSession;
// use serde::{Deserialize, Serialize};
//
// #[derive(Serialize, Deserialize)]
// pub struct ConflictingFlowSteps<'a> {
//     pub to_delete: Vec<FlowStep>,
//     pub to_restore: Vec<FlowStep>,
//
//     #[serde(skip)]
//     pub created_flow_steps: &'a Option<Vec<FlowStep>>,
//
//     #[serde(skip)]
//     pub branch: &'a Branch,
//
//     #[serde(skip)]
//     pub db_session: &'a CachingSession,
// }
//
// impl<'a> ConflictingFlowSteps<'a> {
//     pub fn new(branch: &Branch, db_session: &CachingSession, created_flow_steps: &Option<Vec<FlowStep>>) -> Self {
//         ConflictingFlowSteps {
//             branch,
//             db_session,
//             created_flow_steps,
//             to_delete: vec![],
//             to_restore: vec![],
//         }
//     }
//
//     async fn maybe_original_fs(&self, node_id: Uuid, id: Uuid) -> Result<Option<FlowStep>, NodecosmosError> {
//         FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(node_id, node_id, id)
//             .execute(self.db_session)
//             .await
//             .map_err(|e| {
//                 log::error!("Error finding original flow step: {:?}", e);
//                 NodecosmosError::from(e)
//             })
//     }
//
//     pub async fn execute(&mut self) -> Result<(), NodecosmosError> {
//         let created_flow_step_ids = &self.branch.created_flow_steps;
//
//         let mut idx = 0;
//
//         let mut flow_steps_to_delete = vec![];
//         let mut flow_steps_to_restore = vec![];
//
//         if let Some(branch_flow_steps) = &self.created_flow_steps {
//             for branch_flow_step in branch_flow_steps {
//                 let prev_fs_id = branch_flow_step.prev_flow_step_id;
//                 let next_flow_step_id = branch_flow_step.next_flow_step_id;
//
//                 match (prev_fs_id, next_flow_step_id) {
//                     (Some(prev_fs_id), Some(next_flow_step_id)) => {
//                         let prev_created_on_branch = created_flow_step_ids.contains(&prev_fs_id);
//                         let next_created_on_branch = created_flow_step_ids.contains(&next_flow_step_id);
//                         let prev_fs = branch_flow_steps.get(idx - 1);
//                         let next_flow_step = branch_flow_steps.get(idx + 1);
//                         let node_id = branch_flow_step.node_id;
//
//                         match (prev_created_on_branch, next_created_on_branch) {
//                             (true, true) => {
//                                 // both prev and next are created by branch
//                                 continue;
//                             }
//                             (true, false) => {
//                                 self.handle_prev_branch_next_original(node_id, prev_fs, next_flow_step_id)
//                                     .await?;
//                             }
//                             (false, true) => {
//                                 // next is created by branch
//                                 // 1) delete all that are equal or less than next_flow_step's flow_index and greater
//                                 //    than prev_fs's flow_index
//                                 // 2) restore prev_fs if it's deleted
//                                 self.handle_prev_original_next_branch(prev_fs_id, next_flow_step)
//                                     .await?;
//                             }
//                             (false, false) => {
//                                 // both prev and next are not created by branch
//                                 // 1) restore both prev and next flow steps if they are deleted
//                                 let original_prev_fs = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
//                                     branch_flow_step.node_id,
//                                     branch_flow_step.original_id(),
//                                     prev_fs_id,
//                                 )
//                                 .execute(self.db_session)
//                                 .await?;
//
//                                 let original_nfs = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
//                                     branch_flow_step.node_id,
//                                     branch_flow_step.original_id(),
//                                     next_flow_step_id,
//                                 )
//                                 .execute(self.db_session)
//                                 .await?;
//
//                                 match (original_prev_fs, original_nfs) {
//                                     (Some(original_prev_fs), Some(original_nfs)) => {
//                                         find_flow_step!(
//                                             r#"
//                                             node_id = ?
//                                             AND branch_id = ?
//                                             AND flow_id = ?
//                                             AND flow_index > ?
//                                             AND flow_index < ?
//                                             "#,
//                                             (
//                                                 branch_flow_step.node_id,
//                                                 branch_flow_step.original_id(),
//                                                 prev_fs_id,
//                                                 original_prev_fs.flow_index,
//                                                 original_nfs.flow_index
//                                             )
//                                         )
//                                         .execute(self.db_session)
//                                         .await?
//                                         .try_collect()
//                                         .await?
//                                         .into_iter()
//                                         .for_each(|fs| {
//                                             flow_steps_to_delete.push(fs);
//                                         });
//                                     }
//                                     (Some(_), None) => {
//                                         // we need to restore next flow step
//                                         let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                                             branch_flow_step.node_id,
//                                             branch_flow_step.branch_id,
//                                             next_flow_step_id,
//                                         )
//                                         .execute(self.db_session)
//                                         .await
//                                         .map_err(|e| {
//                                             log::error!("Error finding flow step to restore: {:?}", e);
//                                             e
//                                         })?;
//
//                                         flow_steps_to_restore.push(fs_to_restore);
//                                     }
//                                     (None, Some(_)) => {
//                                         // we need to restore prev flow step
//                                         let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                                             branch_flow_step.node_id,
//                                             branch_flow_step.branch_id,
//                                             prev_fs_id,
//                                         )
//                                         .execute(self.db_session)
//                                         .await
//                                         .map_err(|e| {
//                                             log::error!("Error finding flow step to restore: {:?}", e);
//                                             e
//                                         })?;
//
//                                         flow_steps_to_restore.push(fs_to_restore);
//                                     }
//                                     (None, None) => {
//                                         find_flow_step!(
//                                             r#"
//                                             node_id = ?
//                                             AND branch_id = ?
//                                             AND id IN ?
//                                             "#,
//                                             (
//                                                 branch_flow_step.node_id,
//                                                 branch_flow_step.branch_id,
//                                                 vec![prev_fs_id, next_flow_step_id]
//                                             )
//                                         )
//                                         .execute(self.db_session)
//                                         .await?
//                                         .try_collect()
//                                         .await?
//                                         .into_iter()
//                                         .for_each(|fs| {
//                                             flow_steps_to_restore.push(fs);
//                                         });
//                                     }
//                                 }
//                             }
//                         }
//                     }
//                     (Some(prev_fs_id), None) => {
//                         if created_flow_step_ids.contains(&prev_fs_id) {
//                             continue;
//                         }
//
//                         let original_prev_fs = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
//                             branch_flow_step.node_id,
//                             branch_flow_step.original_id(),
//                             prev_fs_id,
//                         )
//                         .execute(self.db_session)
//                         .await?;
//
//                         if let Some(original_prev_fs) = original_prev_fs {
//                             // delete all after prev_fs_id
//                             find_flow_step!(
//                                 r#"
//                                 node_id = ?
//                                 AND branch_id = ?
//                                 AND flow_id = ?
//                                 AND flow_index > ?
//                                 "#,
//                                 (
//                                     branch_flow_step.node_id,
//                                     branch_flow_step.original_id(),
//                                     original_prev_fs.flow_id,
//                                     original_prev_fs.flow_index
//                                 )
//                             )
//                             .execute(self.db_session)
//                             .await?
//                             .try_collect()
//                             .await?
//                             .into_iter()
//                             .for_each(|fs| {
//                                 flow_steps_to_delete.push(fs);
//                             });
//                         } else {
//                             // restore prev flow step
//                             let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                                 branch_flow_step.node_id,
//                                 branch_flow_step.branch_id,
//                                 prev_fs_id,
//                             )
//                             .execute(self.db_session)
//                             .await
//                             .map_err(|e| {
//                                 log::error!("Error finding flow step to restore: {:?}", e);
//                                 e
//                             })?;
//
//                             flow_steps_to_restore.push(fs_to_restore);
//                         }
//                     }
//                     (None, Some(next_flow_step_id)) => {
//                         if created_flow_step_ids.contains(&next_flow_step_id) {
//                             continue;
//                         }
//
//                         let original_nfs = FlowStep::maybe_find_first_by_node_id_and_branch_id_and_id(
//                             branch_flow_step.node_id,
//                             branch_flow_step.original_id(),
//                             next_flow_step_id,
//                         )
//                         .execute(self.db_session)
//                         .await?;
//
//                         if let Some(original_nfs) = original_nfs {
//                             // delete all before next_flow_step_id
//                             find_flow_step!(
//                                 r#"
//                                 node_id = ?
//                                 AND branch_id = ?
//                                 AND flow_id = ?
//                                 AND flow_index < ?
//                                 "#,
//                                 (
//                                     branch_flow_step.node_id,
//                                     branch_flow_step.original_id(),
//                                     original_nfs.flow_id,
//                                     original_nfs.flow_index
//                                 )
//                             )
//                             .execute(self.db_session)
//                             .await?
//                             .try_collect()
//                             .await?
//                             .into_iter()
//                             .for_each(|fs| {
//                                 flow_steps_to_delete.push(fs);
//                             });
//                         } else {
//                             // restore next flow step
//                             let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                                 branch_flow_step.node_id,
//                                 branch_flow_step.branch_id,
//                                 next_flow_step_id,
//                             )
//                             .execute(self.db_session)
//                             .await
//                             .map_err(|e| {
//                                 log::error!("Error finding flow step to restore: {:?}", e);
//                                 e
//                             })?;
//
//                             flow_steps_to_restore.push(fs_to_restore);
//                         }
//                     }
//                     (None, None) => {
//                         // we need to restore both prev and next flow steps
//                         find_flow_step!(
//                             r#"
//                             node_id = ?
//                             AND branch_id = ?
//                             AND id IN ?
//                             "#,
//                             (
//                                 branch_flow_step.node_id,
//                                 branch_flow_step.branch_id,
//                                 vec![prev_fs_id, next_flow_step_id]
//                             )
//                         )
//                         .execute(self.db_session)
//                         .await?
//                         .try_collect()
//                         .await?
//                         .into_iter()
//                         .for_each(|fs| {
//                             flow_steps_to_restore.push(fs);
//                         });
//                     }
//                 }
//
//                 idx += 1;
//             }
//         }
//
//         if flow_steps_to_delete.is_empty() && flow_steps_to_restore.is_empty() {
//             return Ok(None);
//         }
//
//         Ok(())
//     }
//
//     async fn handle_prev_branch_next_original(
//         &mut self,
//         node_id: Uuid,
//         prev_fs: Option<&FlowStep>,
//         next_fs_id: Uuid,
//     ) -> Result<(), NodecosmosError> {
//         // prev is created by branch
//         // 1) delete all that are equal or greater than prev_fs's flow_index and
//         //    less than next_flow_step's flow_index
//         // 2) restore next_flow_step if it's deleted
//         let original_nfs = self.maybe_original_fs(node_id, next_fs_id).await?;
//
//         match (prev_fs, original_nfs) {
//             (Some(prev_fs), Some(original_nfs)) => {
//                 find_flow_step!(
//                     r#"
//                                             node_id = ?
//                                             AND branch_id = ?
//                                             AND flow_id = ?
//                                             AND flow_index >= ?
//                                             AND flow_index < ?
//                                             "#,
//                     (
//                         prev_fs.node_id,
//                         prev_fs.original_id(),
//                         prev_fs.flow_id,
//                         prev_fs.flow_index,
//                         original_nfs.flow_index
//                     )
//                 )
//                 .execute(self.db_session)
//                 .await?
//                 .try_collect()
//                 .await?
//                 .into_iter()
//                 .for_each(|fs| {
//                     self.to_delete.push(fs);
//                 });
//             }
//             (Some(prev_fs), None) => {
//                 // we need to restore next flow step
//                 let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                     prev_fs.node_id,
//                     prev_fs.branch_id,
//                     next_fs_id,
//                 )
//                 .execute(self.db_session)
//                 .await
//                 .map_err(|e| {
//                     log::error!("Error finding flow step to restore: {:?}", e);
//                     e
//                 })?;
//
//                 self.to_restore.push(fs_to_restore);
//             }
//             _ => {
//                 log::error!("branch created prev_fs is None");
//
//                 return Err(NodecosmosError::InternalServerError(
//                     "branch created prev_fs is None".to_string(),
//                 ));
//             }
//         }
//
//         Ok(())
//     }
//
//     async fn handle_prev_original_next_branch(
//         &mut self,
//         node_id: Uuid,
//         prev_fs_id: Uuid,
//         next_fs: Option<&FlowStep>,
//     ) -> Result<(), NodecosmosError> {
//         // next is created by branch
//         // 1) delete all that are equal or less than next_flow_step's flow_index and greater
//         //    than prev_fs's flow_index
//         // 2) restore prev_fs if it's deleted
//         let original_prev_fs = self.maybe_original_fs(node_id, prev_fs_id).await?;
//
//         match (original_prev_fs, next_fs) {
//             (Some(original_prev_fs), Some(next_fs)) => {
//                 find_flow_step!(
//                     r#"
//                     node_id = ?
//                     AND branch_id = ?
//                     AND flow_id = ?
//                     AND flow_index > ?
//                     AND flow_index < ?
//                     "#,
//                     (
//                         next_fs.node_id,
//                         next_fs.original_id(),
//                         next_fs.flow_id,
//                         original_prev_fs.flow_index,
//                         next_fs.flow_index
//                     )
//                 )
//                 .execute(self.db_session)
//                 .await?
//                 .try_collect()
//                 .await?
//                 .into_iter()
//                 .for_each(|fs| {
//                     self.to_delete.push(fs);
//                 });
//             }
//             (Some(original_prev_fs), None) => {
//                 // we need to restore prev flow step
//                 let fs_to_restore = FlowStep::find_first_by_node_id_and_branch_id_and_id(
//                     original_prev_fs.node_id,
//                     original_prev_fs.branch_id,
//                     prev_fs_id,
//                 )
//                 .execute(self.db_session)
//                 .await
//                 .map_err(|e| {
//                     log::error!("Error finding flow step to restore: {:?}", e);
//                     e
//                 })?;
//
//                 self.to_restore.push(fs_to_restore);
//             }
//             _ => {
//                 log::error!("original_prev_fs is None");
//
//                 return Err(NodecosmosError::InternalServerError(
//                     "original_prev_fs is None".to_string(),
//                 ));
//             }
//         }
//
//         Ok(())
//     }
// }
