### Goals
- Provide a simple, easy to use, performant, and easy to configure CQL ORM mainly focused on ScyllaDB

### Performance consideration of the Charybdis ORM
  - It uses prepared statements with bound values (shard/token aware)
  - It uses serialized values for non-bound query values
  - Basic CRUD queries are constants generated at compile time
  - While it has expressive API it also has close to zero allocations (on its own) in the hot path

### Usage considerations: 
#### Expressive and easy to use
 - Declare model as a struct within src/models dir (Note we use 'src/models' as migration tool expects that)
```rust
// src/modles/user.rs
use charybdis::prelude::*;
use super::udts::Address;

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

- Migrate prev structure by using 'charybdis_cmd/migration' tool:
```bash
# Migration tool analyzes the model/*.rs files runs migrations according to differences between 
# the model definition and database
migrate
```

- Operations

```rust
mod models;
use crate::models::User;


#[tokio::main]
async fn main() {
    // find user
    let user = User::from_json(json).find_by_primary_key(&session).await;
    
    // create
    let user = User::from_json(json).insert(&session).await;
    
    // update
    let user = User::from_json(json).update(&session).await;
    
    // delete
    let user = User::from_json(json).delete(&session).await;
}

```
