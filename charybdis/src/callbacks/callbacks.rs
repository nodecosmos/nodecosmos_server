use crate::errors::CharybdisError;

pub trait Callbacks {
    async fn before_insert(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_insert(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn before_update(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_update(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn before_delete(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
    async fn after_delete(&self) -> Result<(), CharybdisError> {
        Ok(())
    }
}
