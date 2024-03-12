use crate::errors::NodecosmosError;
use crate::models::traits::Id;
use charybdis::model::Model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Uuid;
use futures::StreamExt;
use std::collections::HashMap;

pub trait GroupById<T: Model> {
    async fn group_by_id(self) -> Result<HashMap<Uuid, T>, NodecosmosError>;
}

impl<T: Model + Id> GroupById<T> for CharybdisModelStream<T> {
    async fn group_by_id(mut self) -> Result<HashMap<Uuid, T>, NodecosmosError> {
        let mut map: HashMap<Uuid, T> = HashMap::new();

        while let Some(result) = self.next().await {
            match result {
                Ok(item) => {
                    map.insert(item.id(), item);
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(map)
    }
}
