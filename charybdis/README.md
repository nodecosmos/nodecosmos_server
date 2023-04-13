### 👾 Use Monstrous tandem of Scylla and Charybdis to build your next project
⚠️ **WIP: This project is currently in an experimental stage; It uses built-in async trait support from rust nightly release**

<img src="https://www.scylladb.com/wp-content/uploads/scylla-opensource-1.png" height="250">

* Scylla: NoSQL DB focused on performance
* Charybdis: Thin layer  on top of `scylla_rust_driver` focused on easy of use and performance

### Usage considerations:
- Provide and expressive API for CRUD & Complex Query operations on model as a whole
- Provide easy way to manipulate model partially by using partial_model! macro
- Intelligent migration tool that analyzes the `src/model/*.rs` files and runs migrations according to differences between the model definition and database

### Performance consideration:
- It's build by nightly release, so it uses builtin support for `async/await` in traits
- It uses prepared statements (shard/token aware) -> bind values
- It expects CachingSession as a session type for operations
- Basic CRUD queries are macro generated constants
- While it has expressive API it's thin layer on top of scylla_rust_driver, and it does not introduce any significant overhead

### Usage:

#### Define Tables
Declare model as a struct within `src/models` dir: (Note we use `src/models` as migration tool expects that dir)
```rust
// src/modles/user.rs
use charybdis::prelude::*;
use super::udts::Address;

#[partial_model_generator] // required on top of the charybdis_model macro
#[charybdis_model(table_name = "users", partition_keys = ["id"], clustering_keys = [], secondary_indexes = [])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub address: Address,
}
```

#### For UDTs
Declare udt model as a struct within `src/models/udts` dir:
```rust
// src/models/udts/address.rs
use charybdis::prelude::*;

#[charybdis_udt_model(type_name = "address")]
pub struct Address {
    pub street: Text,
    pub city: Text,
    pub state: Text,
    pub zip: Text,
    pub country: Text,
}
```
#### For Materialized Views
Declare view model as a struct within `src/models/materialized_views` dir:

```rust
use charybdis::prelude::*;

#[charybdis_view_model(table_name="users_by_username", base_table="users", partition_keys=["username"], clustering_keys=["id"])]
pub struct UsersByUsername {
    pub username: Text,
    pub id: Uuid,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
```
Resulting auto-gen migration query will be:
```sql
CREATE MATERIALIZED VIEW IF NOT EXISTS users_by_email
AS SELECT created_at, updated_at, username, email, id
FROM users
WHERE email IS NOT NULL AND id IS NOT NULL
PRIMARY KEY (email, id)
  ```

### Automatic migration with `charybdis_cmd/migrate`:
Smart migration tool that enables you to migrate your models to database without need to write migrations by hand.
It expects `src/models` files and generates migrations based on differences between model definitions and database.

It supports following operations:
- Create new tables
- Create new columns
- Delete columns
- Create secondary indexes
- Delete secondary indexes
- Create UDTs (`src/models/udts`)
- Create materialized views (`src/models/materialized_views`)

🟢 Tables, Types and UDT dropping is not added. If you don't define model within `src/model` dir it will leave 
model as it is.
```bash
cargo install charybdis_cmd/migrate

migrate --host 172.22.0.4 --keyspace nodecosmos
```
⚠️ If you are working with **existing** datasets, before running migration you need to make sure that your **model** definitions structure
matches the database in respect to table names, column names, column types, partition keys, clustering keys
and secondary indexes so you don't alter structure accidentally. If structure is matched, it will not run any migrations.

### Basic Operations:

#### Create:

```rust
mod models;

use charybdis::prelude::*;
use crate::models::user::*;

#[tokio::main]
async fn main() {
  let session: &CachingSession = init_session().await;
  
  // init user
  let id: Uuid = Uuid::new_v4();
  let user: User = User {
    id,
    email: "charybdis@nodecosmos.com".to_string(),
    username: "charybdis".to_string(),
    created_at: DateTime::from(Utc::now()),
    updated_at: DateTime::from(Utc::now()),
    address: Address {
      street: "street".to_string(),
      state: "state".to_string(),
      zip: "zip".to_string(),
      country: "country".to_string(),
      city: "city".to_string(),
    },
  };

  // create
  user.insert(&session).await;
}
```

#### Find:
```rust
  let user = User {id, ..Default::default()};
  let user: User = user.find_by_primary_key(&session).await.unwrap();
  let users: TypedRowIter<User> = user.find_by_partition_key(&session).await.unwrap();
```

#### Update:
```rust
let user = User::from_json(json);

user.username = "scylla".to_string();
user.email = "some@email.com";

user.update(&session).await;
```

#### Delete:
```rust 
  let user = User::from_json(json);

  user.delete(&session).await;
```


### Partial Model Operations:
Use auto generated partial model macro to run operations on subset of the model fields.
This macro generates a new struct with same structure as the original model, but only with provided fields.

<p style="color: #e4a47c">
Note: Partition key fields are required!
</p>

```rust
// auto-generated macro - available in user model
partial_user!(OpsUser, id, username);

let id = Uuid::new_v4();
let user: OpsUser = OpsUser { id, username: "scylla".to_string() };

// insert
user.insert(&session).await;

// get whole user
let user = User {id, ..Default::default()};
let res: User = user.find_by_primary_key(&session).await.unwrap();
```


### Future Plans:
 Add auto query builder and dervies for many possible query variations on primary_key fields:`Like`, `Contains`, `In`, `NotIn`, `Between`, `NotBetween`, `GreaterThan`, `LessThan`, `GreaterThanOrEqual`, `LessThanOrEqual`, `NotEqual`, `Equal`, `NotLike`, `NotContains`, `NotIn`, `NotBetween`, `NotGreaterThan`, `NotLessThan`, `NotGreaterThanOrEqual`, `NotLessThanOrEqual`...
  ```rust 
    #[partial_model_generator]
    #[charybdis_model(table_name = "posts", 
                      partition_keys = ["created_at_day"], 
                      clustering_keys = ["tags", "title"],
                      secondary_indexes = ["id"])]
    pub struct Post {
      pub id: Uuid,
      pub title: Text,
      pub description: Text,
      pub tags: Vec<Text>,
      pub created_at_day: Date,
      pub created_at: Timestamp,
      pub updated_at: Timestamp,
    }
    
    let posts: TypedRowIter<Post> = Post::created_at_day(&today).and().tags_contains(&tag)
      .and().title_like(&param).find(&session).await;
  ```
