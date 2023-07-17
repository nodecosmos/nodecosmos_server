use crate::{CharybdisError, Model, SerializedValues, ValueList};
use scylla::transport::session::TypedRowIter;
use scylla::CachingSession;
use std::fmt::Debug;

// Simple batch for Charybdis models
pub struct CharybdisModelBatch {
    batch: scylla::batch::Batch,
    values: Vec<SerializedValues>,
}

impl CharybdisModelBatch {
    pub fn new() -> Self {
        Self {
            batch: scylla::batch::Batch::default(),
            values: Vec::new(),
        }
    }

    pub fn append_create<T: Model + ValueList>(&mut self, model: &T) -> Result<(), CharybdisError> {
        self.batch.append_statement(T::INSERT_QUERY);
        let values = model.serialized()?;

        self.values.push(values.into_owned());

        Ok(())
    }

    pub fn append_update<T: Model>(&mut self, model: T) -> Result<(), CharybdisError> {
        self.batch.append_statement(T::UPDATE_QUERY);

        let update_values = model
            .get_update_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, T::DB_MODEL_NAME.to_string()))?;

        self.values.push(update_values.into_owned());

        Ok(())
    }

    pub fn append_delete<T: Model + ValueList>(&mut self, model: T) -> Result<(), CharybdisError> {
        self.batch.append_statement(T::DELETE_QUERY);

        let primary_key_values = model
            .get_primary_key_values()
            .map_err(|e| CharybdisError::SerializeValuesError(e, T::DB_MODEL_NAME.to_string()))?;

        self.values.push(primary_key_values.into_owned());

        Ok(())
    }

    pub fn append_deletes<T: Model + ValueList>(
        &mut self,
        mut iter: TypedRowIter<T>,
    ) -> Result<(), CharybdisError> {
        while let Some(model) = iter.next() {
            match model {
                Ok(model) => {
                    let result = self.append_delete(model);
                    if let Err(e) = result {
                        return Err(CharybdisError::from(e));
                    }
                }
                Err(e) => return Err(CharybdisError::from(e)),
            };
        }

        Ok(())
    }
    pub async fn execute(&self, db_session: &CachingSession) -> Result<(), CharybdisError> {
        db_session.batch(&self.batch, self.values.clone()).await?;

        Ok(())
    }
}
