# NodeCosmos Server

NodeCosmos is a collaborative platform for product development and system design. It enables teams to structure
projects, model how things work step by step, and drive evolution through peer-reviewed proposals—all in one place.
Think GitHub for systems and innovation.

## 🔧 Key Features

* ### 🌳 Node Tree
  Structure your product as a tree of nodes. Each node can represent a system or its components, ingredients in a
  recipe,
  or any type of constituent depending on the nature of a project.

* ### 🔁 Flows

  Visually define how each node works from beginning to end, step by step by laying out interactions between
  nodes:  [Lightbulb Flow Sample](https://nodecosmos.com/nodes/0e71060b-000a-42c4-a29d-6afd204d79a1/0e71060b-000a-42c4-a29d-6afd204d79a1/workflow)

* ### 📝 Real-Time Documentation
  Document every Node, Flow, Step, and I/O inline with a real-time collaborative editor—no context-switching required.

* ### 💡 Contribution Requests
  Propose changes to any part of the system with a visual diff and threaded feedback—just like GitHub Pull Requests, but
  for structured systems and processes.

# Installation

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
   > **_NOTE:_** For possible problems with max concurrent request try to set the `aio-max-nr`
   value. Add the following line to the `/etc/sysctl.conf` file:
   > ```shell
   > fs.aio-max-nr = 1048576
   > ```
   >To
   > activate the new setting, run:
   > ```shell
   > sysctl -p /etc/sysctl.conf
   > ```

2) **Login to cqlsh**
   ```shell
   docker exec -it scylla1 cqlsh
   ```
3) **Create keyspace and replication factor 3:**
    ```cassandraql
    CREATE KEYSPACE nodecosmos WITH REPLICATION = { 'class' : 'NetworkTopologyStrategy', 'replication_factor' : 3} AND tablets = { 'enabled': false };
    ```

## Resources-Actions-Models

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
* ### Models (`models/`)
  ORM layer that is used to store core application logic. Each model is table in db. If logic is
  reusable, it should
  utilize traits (`models/traits`). If logic is reusable within single model but for it's partials,
  we should
  define it in `models/traits/<model>`.
* ### Partial Models (`models/<model>/partial_<model>`)
  This is the process of dividing models into segments required by action. In Charybdis we can make
  use of [partial_model](https://github.com/nodecosmos/charybdis?tab=readme-ov-file#partial-model) that returns same
  things as base model but for subset of model fields. Each partial is responsible for a specific task. E.g.
  `UpdateDescriptionNode`, `UpdateProfileImageUser`. One
  benefit of this is to reduce need for full model read before update. Instead, we can read only data required for
  particular action
  update this fields without reading model beforehand. Another advantage of partials is that we can have same traits
  implemented for same model but for different segments. E.g. `S3` trait for `UpdateProfileImageUser` and
  `UpdateCoverImageNode`.

### Note:

Looks like tokio has excessive stack usage in debug
builds. `https://github.com/tokio-rs/tokio/issues/2055` In order to avoid stack overflow, we need to
increase the stack size. This can be done by setting `RUST_MIN_STACK` environment variable. For
example, to set the stack size to 8MB, you can run the following command:

```shell
RUST_MIN_STACK=8388608 cargo run
```

## AWS (Currently only S3 and SES)

As entries in the `credentials` file in the `.aws` directory in your home directory (`~
/.aws/credentials`
on Linux, macOS, and Unix; `%userprofile%\.aws\credentials` on Microsoft Windows):

```
[default]
aws_access_key_id=YOUR-ACCESS-KEY
aws_secret_access_key=YOUR-SECRET-KEY
```

## Mailer (dev)

For development purposes, we are using `maildev` as a mail server. It is a simple SMTP server that
allows you to send and receive emails.

`http://localhost:1080/`
