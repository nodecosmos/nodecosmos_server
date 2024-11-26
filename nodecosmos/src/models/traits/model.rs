use charybdis::model::Model;
use charybdis::operations::Find;
use scylla::CachingSession;
use serde::Deserialize;

use crate::errors::NodecosmosError;

#[derive(Copy, Clone, PartialEq, Deserialize, strum_macros::Display, strum_macros::EnumString)]
pub enum ObjectType {
    Node,
    Workflow,
    Flow,
    FlowStep,
    Io,
}

pub trait Reload: Model
where
    Self: 'static,
{
    async fn reload(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError>
    where
        Self: 'static,
    {
        *self = self.find_by_primary_key().execute(db_session).await?;

        Ok(())
    }
}

impl<T: Model + 'static> Reload for T {}
