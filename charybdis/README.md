### Use Monstrous tandem of Scylla and Charybdis to build your next project
<img src="https://www.scylladb.com/wp-content/uploads/scylla-opensource-1.png" height="250">

* Scylla: NoSQL DB focused on performance
* Charybdis: Thin layer  on top of `scylla_rust_driver` focused on easy of use and performance

### Performance consideration:
  - It's build on top of nightly rust and uses async/await
  - It uses prepared statements with bound values (shard/token aware)
  - CRUD query strings are macro generated and cached
  - While it has expressive API it's very thin layer on top of scylla_rust_driver, and it does not introduce any overhead

### Usage considerations
- Provide and expressive API for CRUD & Query operations on model as a whole
- Provide easy way to manipulate model partially
- Intelligent migration tool that analyzes the model/*.rs files and runs migrations according to differences between the model definition and database

### Usage:

#### Declare model as a struct within src/models dir (Note we use 'src/models' as migration tool expects that)
```rust
// src/modles/user.rs
use charybdis::prelude::*;
use super::udts::Address;

#[partial_model_generator] // required on top of the charybdis_model macro
#[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub password: Text,
    pub hashed_password: Text,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub address: Address,
}
```

#### Migrate prev structure by using 'charybdis_cmd/migration' tool:
```bash
# Migration tool analyzes the model/*.rs files runs migrations according to differences between 
# the model definition and database
migrate
```

#### Basic Operation

```rust
mod models;
use crate::models::user::*;

#[tokio::main]
async fn main() {
    // find user
    let mut user = User::from_json(json);
    user.find_by_primary_key(&session).await; // mutates user with data from db
    
    // create
    let user = User::from_json(json).insert(&session).await;
    
    // update
    let user = User::from_json(json).update(&session).await;
    
    // delete
    let user = User::from_json(json).delete(&session).await;
}

```
#### Partial Model Operation

```rust
// auto-generated macro helper
partial_user!(PartialUser, id, username);

#[tokio::main]
async fn main() {
    // find by partial user
    let mut p_user = PartialUser { id: user.id, username: user.username };
    p_user.find_by_primary_key(&session).await;
    
    let response_json = p_user.to_json();
    
    // update by partial user
    let p_user = PartialUser { id: user.id, username: user.username };
    p_user.update(&session).await;
    
    // delete by partial user
    partial_user!(PartialUser, id);
    
    let mut p_user = PartialUser { id: user.id };
    p_user.delete(&session).await;
}
```


```rust

#[macro_export]
macro_rules! partial_user {
    ($struct_name:ident, $($field:ident),*) => {
        #[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
        pub struct $struct_name {
            $(pub $field: field_type!($field),)*
        }
    };
}

#[macro_export]
macro_rules! field_type {
    (id) => { Uuid };
    (username) => { Text };
    (password) => { Text };
    (hashed_password) => { Text };
    (email) => { Text };
    (created_at) => { Timestamp };
    (updated_at) => { Timestamp };
    (address) => { Address };
}

```