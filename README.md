
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
   For possible problems with max concurrent request check this [link](https://sort.veritas.com/public/documents/HSO/2.0/linux/productguides/html/hfo_admin_rhel/ch04s03.htm)

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
