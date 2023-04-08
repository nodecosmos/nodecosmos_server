welcome to #nodecosmos ![](https://via.placeholder.com/15/c5f015/000000?text=+) ![](https://via.placeholder.com/15/f03c15/000000?text=+)
---

1) Install ScyllaDB Using Docker Compose
   ```shell
   docker-compose up
   ```
2) Login to cqlsh
   ```shell
   docker exec -it scylla1 cqlsh
   ```
    * For possible problems with max concurent request check this [link](https://sort.veritas.com/public/documents/HSO/2.0/linux/productguides/html/hfo_admin_rhel/ch04s03.htm) or [aio-mx-nr.txt](https://github.com/microcosmos-org/microcosmos/blob/master/doc/scylla/aio-mx-nr.txt) file in our repo.

3) create keyspace and replication factor 3
    ```cassandraql
     CREATE KEYSPACE nodecosmos WITH REPLICATION = { 'class' : 'NetworkTopologyStrategy', 'replication_factor' : 3};
    ```
