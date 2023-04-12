// use scylla::{CachingSession, QueryResult};
// use crate::errors::CharybdisError;
//
// use crate::model::Model;
// use crate::prelude::SerializedValues;
// use crate::iterator::{CharTypedRowIter, IntoCharTypedRows};
//
// pub trait FindByPartitionKey {
//     async fn find_by_partition_key(&self, session: &CachingSession)
//                                    -> Result<CharTypedRowIter<Self>, CharybdisError> where Self: Model;
// }
//
// impl <T:  Model> FindByPartitionKey for T {
//     async fn find_by_partition_key(&self, session: &CachingSession) -> Result<CharTypedRowIter<Self>, CharybdisError> {
//         let get_partition_key_values: SerializedValues = self.get_partition_key_values();
//
//         let result: QueryResult = session
//             .execute(Self::FIND_BY_PARTITION_KEY_QUERY, get_partition_key_values)
//             .await.map_err(|e| CharybdisError::QueryError(e))?;
//
//         match result.rows {
//             Some(rows) => {
//                 let typed_rows: CharTypedRowIter<Self> = rows.into_typed();
//                 Ok(typed_rows)
//             }
//             None => {
//                 Err(CharybdisError::NotFoundError(Self::DB_MODEL_NAME.to_string()))
//             }
//         }
//     }
// }
