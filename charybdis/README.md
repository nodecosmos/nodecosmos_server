### 👾 Use Monstrous tandem of Scylla and Charybdis to build your next project
⚠️ **WIP: This project is currently in an experimental stage; It uses built-in async trait support from rust nightly release**

<img src="https://www.scylladb.com/wp-content/uploads/scylla-opensource-1.png" height="250">

#### 👾  Charybdis is a thin ORM layer on top of `scylla_rust_driver` focused on easy of use and performance

### Usage considerations:
- Provide and expressive API for CRUD & Complex Query operations on model as a whole
- Provide easy way to manipulate model partially by using automatically generated `partial_<model>!` macro
- Provide easy way to write complex queries by using automatically generated `find_<model>_query!` macro
- Automatic migration tool that analyzes the `src/model/*.rs` files and runs migrations according to differences between the model definition and database
- It works well with optional fields, and it's possible to use `Option<T>` as a field type, automatic migration
tool will detect type within option and create column with that type

### Performance consideration:
- It's build by nightly release, so it uses builtin support for `async/await` in traits
- It uses prepared statements (shard/token aware) -> bind values
- It expects CachingSession as a session type for operations
- Basic CRUD queries are macro generated constants
- By using `find_<model>_query!` macro you can write complex queries that are also generated at compile time
- While it has expressive API it's thin layer on top of scylla_rust_driver, and it does not introduce any significant overhead

### Usage:

#### Define Tables
Declare model as a struct within `src/models` dir: (Note we use `src/models` as migration tool expects that dir)
```rust
// src/modles/user.rs
use charybdis::prelude::*;
use super::udts::Address;

#[partial_model_generator] // required on top of the charybdis_model macro to generate partial_user helper
#[charybdis_model(table_name = "users", 
                  partition_keys = ["id"], 
                  clustering_keys = [], 
                  secondary_indexes = [])]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub address: Option<Address>,
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

#[charybdis_view_model(table_name="users_by_username",
                       base_table="users",
                       partition_keys=["username"],
                       clustering_keys=["id"])]
pub struct UsersByUsername {
    pub username: Text,
    pub id: Uuid,
    pub email: Text,
    pub created_at: Option<Timestamp>,
    pub updated_at: Option<Timestamp>,
}

```
Resulting auto-generated migration query will be:
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
db structure as it is.
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
    address: Some(
        Address {
            street: "street".to_string(),
            state: "state".to_string(),
            zip: "zip".to_string(),
            country: "country".to_string(),
            city: "city".to_string(),
        }
    ),
  };

  // create
  user.insert(&session).await;
}
```

#### Find:
```rust
  let user = User {id, ..Default::default()};
  // primary_key results in single row
  let user: User = user.find_by_primary_key(&session).await.unwrap();
  // partition_key results in multiple rows
  let users: TypedRowIter<User> = user.find_by_partition_key(&session).await.unwrap();

  // or
  let query = find_user_query!("id = ?");
  let users: TypedRowIter<User> = User::find(&session, query, (id,)).await.unwrap();

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

📝 Each of operations will do filtering based on primary key fields that will be taken from the model struct.

### Partial Model Operations:
Use auto generated `partial_<model>!` macro to run operations on subset of the model fields.
This macro generates a new struct with same structure as the original model, but only with provided fields.
It can be used to run **find** operations on records based on mandatory partition keys and provided clustering keys.

📝 Partition key fields are required for **read** operations, while whole primary key fields are required for 
**update**, **insert** and **delete** operations!

```rust
// auto-generated macro - available in user model
partial_user!(OpsUser, id, username);

let id = Uuid::new_v4();
let user: OpsUser = OpsUser { id, username: "scylla".to_string() };

// we can have same operations as on base model
user.insert(&session).await;
user.update(&session).await;

user.delete(&session).await;

// get partial user
let user: OpsUser = user.find_by_primary_key(&:session).await.unwrap();

// get whole user by primary key from primary_key
let user = User {id, ..Default::default()};
let res: User = user.find_by_primary_key(&session).await.unwrap();
```

### View Operations:
```rust
let mut user_by_username: UsersByUsername = UsersByUsername::new();
user_by_username.username = "test_username".to_string();

let users_by_username: TypedRowIter<UsersByUsername> = user_by_username
    .find_by_partition_key(&session)
    .await
    .unwrap();

for user in users_by_username {
    println!("{:?}", user);
}

// custom queries
let query = find_users_by_username_query!("username = ?");

let users_by_username: TypedRowIter<UsersByUsername> =
    UsersByUsername::find(&session, query, ("test_username",))
        .await
        .unwrap();

for user in users_by_username {
    println!("{:?}", user);
}
```

### Custom filtering:
Let's say we have a model:
```rust 
#[partial_model_generator]
#[charybdis_model(table_name = "posts", 
                  partition_keys = ["created_at_day"], 
                  clustering_keys = ["title"],
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
```
We get automatically generated `find_post_query!` macro that follows convention `find_<struct_name>_query!`.
It can be used to create custom filtering clauses like:

```rust
let created_at_day = chrono::Utc::now().day();
let title = "some title";

// automatically generated macro rule that follows convention find_
let query = find_post_query!("created_at_day = ? AND title = ?");

let posts: TypedRowIter<Post> = Post::find(&session, query, (created_at, updated_at)).await.unwrap();
```

Also if we are working with **partial** models, we can use `find_<struct_name>_query` and `find` methods on partial models:
```rust
partial_post!(OpsPost, id, title, created_at_day);

// automatically generated macro
let query = find_ops_post_query!("created_at_day = ? AND title = ?");

OpsPost::find(&session, query, (created_at, updated_at)).await.unwrap();
```

**find_<struct_name>_query!** macro comes with some benefits like:
- correct fields order in select clause
- we don't do string interpolation at runtime as it's static string
- easy of use.

### Some of the important limitations:
- Fields that can be null have to be defined within `Option` or it will raise an error when parsing queries
- Batch operations are not supported yet


### Future plans:
- [ ] Add tests
- [ ] Write `modelize` command to generate `src/models/*` structs from existing database
- [ ] Add --drop flag to migrate command to drop tables, types and UDTs if they are not defined in `src/models`
- [ ] Add support for batch operations e.g. `insert_all`, `update_all`, etc...
