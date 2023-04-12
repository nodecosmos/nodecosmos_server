use scylla::FromRow;
use crate::prelude::{SerializedValues, CharybdisError};

///
/// Model is a trait that defines the basic structure of a table in the database.
/// It is used to generate the necessary code for the charybdis orm.
/// The trait is implemented by the user and the macro generates the necessary code.
/// The macro is used in the following way:
/// ```rust
/// use charybdis::prelude::*;
///
/// #[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes=[])]
/// pub struct User {
///     pub id: Uuid,
///     pub username: Text,
///     pub password: Text,
///     pub hashed_password: Text,
///     pub email: Text,
///     pub created_at: Timestamp,
///     pub updated_at: Timestamp,
/// }
/// ```
///
/// These structure is used by smart `migration` tool that automatically migrate the database schema from the code.
/// It detects changes in the model and automatically applies the changes to the database.
///
/// If you have migration package installed, you can run the `migrate` command to automatically
/// migrate the database schema without having to write any CQL queries.
///
pub trait Model: FromRow + Sized + Default {
    const DB_MODEL_NAME: &'static str;

    const PARTITION_KEYS: &'static [&'static str];
    const CLUSTERING_KEYS: &'static [&'static str];
    const PRIMARY_KEY: &'static [&'static str];
    const SECONDARY_INDEXES: &'static [&'static str];

    const FIND_BY_PRIMARY_KEY_QUERY: &'static str;
    const FIND_BY_PARTITION_KEY_QUERY: &'static str;
    const INSERT_QUERY: &'static str;
    const UPDATE_QUERY: &'static str;
    const DELETE_QUERY: &'static str;

    // associated
    // fn from_row(row: &Row) -> Result<Self, CharybdisError>;

    // methods
    fn get_primary_key_values(&self) -> SerializedValues;
    fn get_partition_key_values(&self) -> SerializedValues;
    fn get_clustering_key_values(&self) -> SerializedValues;
}

pub trait MaterializedView: FromRow + Sized + Default {
    const DB_MODEL_NAME: &'static str;

    const PARTITION_KEYS: &'static [&'static str];
    const CLUSTERING_KEYS: &'static [&'static str];
    const PRIMARY_KEY: &'static [&'static str];

    const FIND_BY_PRIMARY_KEY_QUERY: &'static str;
    const FIND_BY_PARTITION_KEY_QUERY: &'static str;

    fn get_primary_key_values(&self) -> SerializedValues;
    fn get_partition_key_values(&self) -> SerializedValues;
    fn get_clustering_key_values(&self) -> SerializedValues;
}

pub trait Udt: FromRow + Sized {
    const DB_MODEL_NAME: &'static str;
}
