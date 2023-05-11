## 👾 Use Monstrous tandem of Scylla and Charybdis to build your next project
⚠️ **WIP: This project is currently in an experimental stage; It uses built-in async trait support from rust nightly release**

<img src="https://www.scylladb.com/wp-content/uploads/scylla-opensource-1.png" height="250">

#### Charybdis is a thin ORM layer on top of `scylla_rust_driver` focused on easy of use and performance

## Usage considerations:
- Provide and expressive API for CRUD & Complex Query operations on model as a whole
- Provide easy way to manipulate subset of model fields by using automatically generated `partial_<model>!` macro
- Provide easy way to write complex queries by using automatically generated `find_<model>_query!` macro
- Automatic migration tool that analyzes the `src/model/*.rs` files and runs migrations according to differences between the model definition and database
- It works well with optional fields, and it's possible to use `Option<T>` as a field type, automatic migration
tool will detect type within option and create column with that type

## Performance consideration:
- It's build by nightly release, so it uses builtin support for `async/await` in traits
- It uses prepared statements (shard/token aware) -> bind values
- It expects `CachingSession` as a session arg for operations
- Basic CRUD queries are macro generated str constants
- By using `find_<model>_query!` macro you can write complex queries that are also generated at compile time as `&'static str`
- While it has expressive API it's thin layer on top of scylla_rust_driver, and it does not introduce any significant overhead

## Table of Contents
- [Charybdis Models](#charybdis-models)
  - [Define Tables](#define-tables)
  - [Define UDTs](#Define-UDT)
  - [Define Materialized Views](#Define-Materialized-Views)
- [Automatic migration tool with `charybdis_cmd/migrate`](#automatic-migration)
- [Basic Operations](#basic-operations)
  - [Create](#create)
  - [Find](#find)
  - [Update](#update)
  - [Delete](#delete)
- [Partial Model Operations](#partial-model-operations)
- [View Operations](#view-operations)
- [Custom filtering](#custom-filtering)
- [Callbacks](#callbacks)
- [Batch Operations](#batch-operations)
- [As Native](#as-native)
- [Collection queries](#collection-queries)
- [Roadmap](#Roadmap)

## Charybdis Models


### Define Tables

Declare model as a struct within `src/models` dir:
```rust
// src/modles/user.rs
use super::udts::Address;
use charybdis::{charybdis_model, partial_model_generator, Text, Timestamp, Uuid};

#[partial_model_generator] // required on top of the charybdis_model macro to generate partial_user helper
#[charybdis_model(
    table_name = "users",
    partition_keys = ["id"],
    clustering_keys = [],
    secondary_indexes = []
)]
pub struct User {
    pub id: Uuid,
    pub username: Text,
    pub email: Text,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub address: Option<Address>,
}
```
(Note we use `src/models` as automatic migration tool expects that dir)

### Define UDT
Declare udt model as a struct within `src/models/udts` dir:
```rust
// src/models/udts/address.rs
use charybdis::*;

#[charybdis_udt_model(type_name = "address")]
pub struct Address {
    pub street: Text,
    pub city: Text,
    pub state: Text,
    pub zip: Text,
    pub country: Text,
}
```
### Define Materialized Views
Declare view model as a struct within `src/models/materialized_views` dir:

```rust
use charybdis::*;

#[charybdis_view_model(
    table_name="users_by_username",
    base_table="users",
    partition_keys=["username"],
    clustering_keys=["id"]
)]
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

## Automatic migration with `charybdis_cmd/migrate`:
<a name="automatic-migration"></a>
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

migrate --hosts <host> --keyspace <your_keyspace>
```

⚠️ If you are working with **existing** datasets, before running migration you need to make sure that your **model** 
definitions structure matches the database in respect to table names, column names, column types, partition keys, 
clustering keys and secondary indexes so you don't alter structure accidentally.
If structure is matched, it will not run any migrations. As mentioned above, 
in case there is no model definition for table, it will **not** drop it.

## Basic Operations:

### Create

```rust
mod models;

use charybdis::*;
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

### Find

#### `find_by_primary_key`
```rust
  let user = User {id, ..Default::default()};
  let user: User = user.find_by_primary_key(&session).await.unwrap();
```
`find_by_partition_key`
```rust
  // partition_key results in multiple rows
  let users: TypedRowIter<User> = user.find_by_partition_key(&session).await.unwrap();
```
`find_model_query!` & `find`
```rust
  let query = find_user_query!("id = ?");
  let users: TypedRowIter<User> = User::find(&session, query, (id,)).await.unwrap();
```
`find_one`
```rust
let user = User::find_one(&session, find_user_query!("id = ?"), (id,)).await?;
```

### Update
```rust
let user = User::from_json(json);

user.username = "scylla".to_string();
user.email = "some@email.com";

user.update(&session).await;
```

### Delete
```rust 
  let user = User::from_json(json);

  user.delete(&session).await;
```

📝 Each of operations will do filtering based on primary key fields that will be taken from the model struct.

## Partial Model Operations:
Use auto generated `partial_<model_name>!` macro to run operations on subset of the model fields.
This macro generates a new struct with same structure as the original model, but only with provided fields.
Limitation is that it requires whole primary key fields to be added.
Macro is defined by using `#[partial_model_generator]`.
```rust
// auto-generated macro - available in crate::models::user
partial_user!(OpsUser, id, username);

let id = Uuid::new_v4();
let op_user: OpsUser = OpsUser { id, username: "scylla".to_string() };

// we can have same operations as on base model
// INSERT into users (id, username) VALUES (?, ?)
op_user.insert(&session).await;

// UPDATE users SET username = ? WHERE id = ?
op_user.update(&session).await;

// DELETE FROM users WHERE id = ?
op_user.delete(&session).await;

// get partial user
let op_user: OpsUser = user.find_by_primary_key(&:session).await.unwrap();

// get whole user by primary key
let user = op_user.as_native().find_by_primary_key(&session).await.unwrap();
```

Note that if you have custom attributes on your model fields,
they will be automatically added to partial fields.

```rust
#[partial_model_generator]
#[charybdis_model(
    table_name = "nodes",
    partition_keys = ["root_id"],
    clustering_keys = ["id"],
    secondary_indexes = []
)]
pub struct Node {
    // descendable
    pub id: Uuid,

    #[serde(rename = "rootId")]
    pub root_id: Uuid, 
    pub title: String,
}

partial_node!(PartialNode, id, root_id);
```

`PartialNode` will include serde attributes `#[serde(rename = "rootId")]` from `Node` model.

## View Operations:
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

## Custom filtering:
Let's say we have a model:
```rust 
#[partial_model_generator]
#[charybdis_model(
    table_name = "posts", 
    partition_keys = ["created_at_day"], 
    clustering_keys = ["title"],
    secondary_indexes = ["id"]
)]
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
// automatically generated macro rule
let query = find_post_query!("created_at_day = ? AND title = ?");

let created_at_day = Utc::now().date_naive();
let title = "some title";

let posts: TypedRowIter<Post> = Post::find(&session, query, (created_at_day, title)).await.unwrap();
```

For Custom update queries we have `update_<struct_name>_query!` macro.

For value list we first pass the values that we want to update, 
and then we provide primary key values.
```rust
let query = update_post_query!("description = ?");
let res = execute(&session, query, (new_desc, created_at_day, title))
    .await
    .unwrap();
```

Also, if we are working with **partial** models, we can use `find_<struct_name>_query` and
`update_<struct_name>_query!`, rules

models:
```rust
partial_post!(OpsPost, id, title, created_at_day);

// automatically generated macro
let query = find_ops_post_query!("created_at_day = ? AND title = ?");
let posts: TypedRowIter<OpsPost> = OpsPost::find(&session, query, (created_at, updated_at))
    .await
    .unwrap();
```

**find_<struct_name>_query!** macro comes with some benefits like:
- correct fields order in select clause
- it builds query as `&'static str`
- easy of use


## Callbacks
We can define callbacks that will be executed before and after certain operations.

```rust
use charybdis::*;

#[charybdis_model(
    table_name = "users", 
    partition_keys = ["id"], 
    clustering_keys = [""],
    secondary_indexes = ["username", "email"]
)]
pub struct User {
    ...
}

impl User {
  pub async fn find_by_username(&self, session: &CachingSession) -> Option<User> {
    let query = find_user_query!("username = ?");

    Self::find_one(session, query, (&self.username,)).await.ok()
  }

  pub async fn find_by_email(&self, session: &CachingSession) -> Option<User> {
    let query = find_user_query!("email = ?");

    Self::find_one(session, query, (&self.email,)).await.ok()
  }
}

impl Callbacks for User {
  async fn before_insert(&self, session: &CachingSession) -> Result<(), CharybdisError> {
    if self.find_by_username(session).await.is_some() {
      return Err(CharybdisError::ValidationError((
        "username".to_string(),
        "is taken".to_string(),
      )));
    }

    if self.find_by_email(session).await.is_some() {
      return Err(CharybdisError::ValidationError((
        "email".to_string(),
        "is taken".to_string(),
      )));
    }

    Ok(())
  }
}
```
Possible callbacks:
- `before_insert`
- `after_insert`
- `before_update`
- `after_update`
- `before_delete`
- `after_delete`

In order to trigger callback, instead of calling `insert` method on model, we can call `insert_cb`:
```rust
let post = Post::from_json(json);
let res = post.insert_cb(&session).await;
match res {
        Ok(_) => println!("success"),
        Err(e) => match e {
            CharybdisError::ValidationError((field, reason)) => {
                println!("validation error: {} {}", field, reason)
            }
            _ => println!("error: {:?}", e),
        },
    }
```

## Batch Operations

We can execute batch operations by using combination of native driver `Batch` and queries
generated by `charybdis`:

```rust
  let id = Uuid::new_v4();
  let mut user = User {
      id,
      username: "initial_username".to_string(),
      email: "test@nodecosmos.com".to_string(),
      password: "test".to_string(),
      hashed_password: "test".to_string(),
      first_name: "test".to_string(),
      last_name: "test".to_string(),
      created_at: Utc::now(),
      updated_at: Utc::now(),
      address: None,
  };

  partial_user!(UpdateUser, id, username);

  let mut user2 = UpdateUser {
      id,
      username: "updated_username".to_string(),
  };

  let mut batch: Batch = Default::default();

  batch.append_statement(User::INSERT_QUERY);
  batch.append_statement(UpdateUser::UPDATE_QUERY);

  let user2_update_values = user2.get_update_values().unwrap();

  let batch_res = session
      .batch(&batch, (&user, user2_update_values))
      .await
      .unwrap();

  let user = user.find_by_primary_key(&session).await.unwrap();

  assert_eq!(user.username, "updated_username".to_string());
```
## As Native
In case you need native model in order to run calculations or other operations, you can use `as_native` method:
```rust
partial_user!(UpdateUser, id, username);

let mut user = UpdateUser {
    id,
    username: "updated_username".to_string(),
};

let native_user = user.as_native().find_by_primary_key(&session).await.unwrap();

// action that requires native model
authorize_user(&native_user);

```

## Collection queries
For every field that is defined with `List<T> `type or `Set<T>`, we have macro generated queries:
- `PUSH_TO_<field_name>_QUERY`
- `PULL_FROM_<field_name>_QUERY`
that can be used to push or pull elements from collection.

```rust
pub struct User {
    id: Uuid,
    tags: Set<String>,
    edited_posts: List<Uuid>,
}

let query = User::PUSH_TO_TAGS_QUERY;
execute(query, ("new_tag", &user.id)).await;

let query = User::PULL_FROM_EDITED_POSTS_QUERY;
execute(query, (uuid, &user.id)).await;
```

## Limitations:
1) `partial_models` don't implement same callbacks defined on base model so 
    `insert_cb`, `update_cb`, `delete_cb` will not work on partial models unless callbacks are
    manually implemented for partial models e.g.
    ```rust
    partial_user!(UpdateUser, id, first_name, last_name, updated_at, address);
    
    impl Callbacks for UpdateUser {
        async fn before_update(&mut self, _: &CachingSession) -> Result<(), CharybdisError> {
            self.updated_at = Some(Utc::now());
            Ok(())
        }
    }
    ```
    or you can always implement macro helper for this use case
    ```rust
    macro_rules! set_updated_at_cb {
        ($struct_name:ident) => {
            impl Callbacks for $struct_name {
                async fn before_update(
                    &mut self,
                    _session: &CachingSession,
                ) -> Result<(), CharybdisError> {
                    self.updated_at = Some(Utc::now());
                    Ok(())
                }
            }
        };
    }
    
    pub(crate) use set_updated_at_cb;
    ```
    and then only use it in partial model definition:
    ```rust
    partial_user!(UpdateUserFirstName, id, first_name);
    set_updated_at_cb!(UpdateUser);
    
    partial_user!(UpdateUserLastName, id, last_name);
    set_updated_at_cb!(UpdateUser);
    ```

2) `partial_models` require complete primary key

## Roadmap:
- [ ] Add tests
- [ ] Write `modelize` command to generate `src/models/*` structs from existing database
- [ ] Add --drop flag to migrate command to drop tables, types and UDTs if they are not defined in 
`src/models`
