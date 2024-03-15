use charybdis::types::Uuid;

/// implemented by #[derive(Id)]
pub trait Id {
    fn id(&self) -> Uuid;
}

/// implemented by #[derive(RootId)]
pub trait RootId {
    fn root_id(&self) -> Uuid;
}
