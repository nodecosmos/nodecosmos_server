welcome to #nodecosmos ![](https://via.placeholder.com/15/c5f015/000000?text=+) ![](https://via.placeholder.com/15/f03c15/000000?text=+)
---

1) Install ScyllaDB Using Docker Compose
   ```shell
   docker-compose up
   ```
   It usually takes some time for initialization, compaction and other stuff. You can check the status of the cluster by running:
   ```shell
    docker-compose exec scylla1 nodetool status
    ```
   It's important to wait until initialization and complete for all nodes unless there are some
   "fatal errors" in the logs.

   **For possible problems with max concurrent request check this [link](https://sort.veritas.com/public/documents/HSO/2.0/linux/productguides/html/hfo_admin_rhel/ch04s03.htm)**

2) Login to cqlsh
   ```shell
   docker exec -it scylla1 cqlsh
   ```

3) create keyspace and replication factor 3
    ```cassandraql
     CREATE KEYSPACE nodecosmos WITH REPLICATION = { 'class' : 'NetworkTopologyStrategy', 'replication_factor' : 3};
    ```
