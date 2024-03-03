paping---

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

##### TODO:

- [ ] Scylla Monitoring Stack
- [ ] Nodecosmos
- [ ] ElasticSearch
- [ ] Redis

## CASM - Clients-Actions(API)-Segmentation-Models

* ### Clients (ATM `App.rs` but to be moved to `clients/`)
  They represent data structures that are alive during program runtime. They are usually related in one way or another
  to external services sessions: e.g. ScyllaDb, Redis, ElasticSearch, etc.
* ### Actions (`api/`)
  They represent entry point of the request. They are responsible for parsing request or triggering
  model or model-segment specific logic and returning the response.
  E.g. `update_node_description`, `update_user_profile_image`, etc.
* ### Segmentation
  This is the process of dividing models into segments required by action. In Charybdis we can make use
  of `partial<models>` that returns same things as base model but for subset of model fields. Each segment is
  responsible for a specific task. E.g. `UpdateDescriptionNode`, `UpdateProfileImageUser`. One benefit of segmentation
  is
  to reduce need for full model read before update. Instead we can read only data required for authorization and
  update this fields without reading model beforehand. Another benefit of segmentation is that we can have same traits
  implemented for same model but for different segments. E.g. `S3` trait for `UpdateProfileImageUser`
  and `UpdateCoverImageUser`.
* ### Models
  They represent data structures that are used to model core application data and implement the business logic. If
  business logic is reusable, it should utilize traits (`models/traits`). And accordingly the traits should be
  implemented by model or the segment. E.g. In case same model has multiple s3 related
  fields `cover_image`, `profile_image` we can have a separate trait for `partial_<model_name>` and implement `S3` trait
  in it.
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
