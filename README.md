1) **Install related services with docker compose**
   ```shell
   docker-compose up
   ``` 
   This will install `ScyllaDB`, `Redis`, `ElasticSearch` and `Kibana`.
   Scylla cluster usually takes some time for initialization, compaction and other processes.
   You can check the status of the scylla cluster by running:
   ```shell
   docker exec scylla1 nodetool status
   ```
   For possible problems with max concurrent request check
   this [link](https://sort.veritas.com/public/documents/HSO/2.0/linux/productguides/html/hfo_admin_rhel/ch04s03.htm)
2) **Login to cqlsh**
   ```shell
   docker exec -it scylla1 cqlsh
   ```
3) **Create keyspace and replication factor 3:**
    ```cassandraql
    CREATE KEYSPACE nodecosmos WITH REPLICATION = { 'class' : 'NetworkTopologyStrategy', 'replication_factor' : 3};
    ```
4) **Install our automatic migration tool**
   ```shell
   cargo install charybdis-migrate
   ```
5) **Run migration**
   ```shell
   migrate --host localhost:9042 --keyspace nodecosmos
   ```

##### TODO:

- [ ] Scylla Monitoring Stack``

## Resources-Actions-Segmentation-Models

* ### Resources (`resources/`)
  They represent data structures that are alive during program runtime. They are usually related to
  external services
  sessions: e.g. `ScyllaDb`, `Redis`, `ElasticSearch`, etc. But it can also be app specific e.g.
  `SseBroadcast`, `Locker`, `DescriptionWsPool`, etc.
* ### Actions (`api/`)
  They represent entry point of the request. They are responsible for parsing request or triggering
  model or
  model-segment specific logic and returning the response.
  E.g. `update_node_description`, `update_user_profile_image`,
  etc.
* ### Segmentation (`models/<model>/partial_<model>`)
  This is the process of dividing models into segments required by action. In Charybdis we can make
  use
  of `partial<model>` that returns same things as base model but for subset of model fields. Each
  segment is
  responsible for a specific task. E.g. `UpdateDescriptionNode`, `UpdateProfileImageUser`. One
  benefit of segmentation
  is to reduce need for full model read before update. Instead we can read only data required for
  authorization and
  update this fields without reading model beforehand. Another advantage of segmentation is that we
  can have same traits
  implemented for same model but for different segments. E.g. `S3` trait
  for `UpdateProfileImageUser`
  and `UpdateCoverImageUser`.
* ### Models (`models/`)
  ORM layer that is used to store core application logic. Each model is table in db. If logic is
  reusable, it should
  utilize traits (`models/traits`). If logic is reusable within single model but for it's partials,
  we should
  define it in `models/traits/<model>`.
   ```rust
   #[charybdis_model(
     table_name = users,
     partition_keys = [id],
     clustering_keys = [],
   )]
   pub struct User {
     pub id: Uuid,
     pub username: String,
     pub email: String,
     pub password: String,
     pub profile_image: Option<String>,
     pub cover_image: Option<String>,
     pub created_at: DateTime<Utc>,
     pub updated_at: DateTime<Utc>,
   }
  
   partial_user!(UpdateProfileImageUser, id, profile_image);
   partial_user!(UpdateCoverImageUser, id, cover_image);
  
   impl S3 for UpdateProfileImageUser {
     fn s3_bucket() -> String {
       "user-profile-images".to_string()
     }
   }
  
    impl S3 for UpdateCoverImageUser {
      fn s3_bucket() -> String {
         "user-cover-images".to_string()
      }
    }
   
   ```

### Note:

Looks like tokio has excessive stack usage in debug
builds. `https://github.com/tokio-rs/tokio/issues/2055` In order to avoid stack overflow, we need to
increase the stack size. This can be done by setting `RUST_MIN_STACK` environment variable. For
example, to set the stack size to 8MB, you can run the following command:

```shell
RUST_MIN_STACK=8388608 cargo run
```

# AWS

As entries in the `credentials` file in the `.aws` directory in your home directory (`~
/.aws/credentials`
on Linux, macOS, and Unix; `%userprofile%\.aws\credentials` on Microsoft Windows):

```shell
[default]
  aws_access_key_id=YOUR-ACCESS-KEY
  aws_secret_access_key=YOUR-SECRET-KEY
```
