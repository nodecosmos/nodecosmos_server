use std::collections::HashMap;

use charybdis::model::Model;
use charybdis::stream::CharybdisModelStream;
use charybdis::types::Uuid;
use futures::StreamExt;

use crate::errors::NodecosmosError;
use crate::models::traits::{Id, ObjectId};

pub trait GroupById<T: Model + Id> {
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

impl<T: Model + Id> GroupById<T> for Vec<T> {
    async fn group_by_id(self) -> Result<HashMap<Uuid, T>, NodecosmosError> {
        let mut map: HashMap<Uuid, T> = HashMap::new();

        for item in self {
            map.insert(item.id(), item);
        }

        Ok(map)
    }
}

pub trait GroupByObjectId<T: Model + ObjectId> {
    async fn group_by_object_id(self) -> Result<HashMap<Uuid, T>, NodecosmosError>;
}

impl<T: Model + ObjectId> GroupByObjectId<T> for CharybdisModelStream<T> {
    async fn group_by_object_id(mut self) -> Result<HashMap<Uuid, T>, NodecosmosError> {
        let mut map: HashMap<Uuid, T> = HashMap::new();

        while let Some(result) = self.next().await {
            match result {
                Ok(item) => {
                    map.insert(item.object_id(), item);
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(map)
    }
}
