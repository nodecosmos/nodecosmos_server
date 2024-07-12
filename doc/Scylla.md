## ![](https://via.placeholder.com/20/e91e63/000000?text=+) Monitoring Stack

### Scylla manager

On dedicated monitoring node we need to install `Scylla Manager`.

###### ⚠️ Scylla manager has its own scylladb. Its important to separate that node from the cluster on the main nodes.

##### Scylla Manager Config:

1. On each DB node we need to install `scylla-manager-agent` service.
2. On one of the DB nodes we can use `scyllamgr_auth_token_gen` to generate one token.
3. On each node update `auth_token` key within `/etc/scylla-manager-agent/scylla-manager-agent.yaml`
   file.
4. Start `scylla-manager-agent` service.
5. On scylla manager node run: `sctool cluster add --host <node-ip> --name dev --auth-token <token>`
6. Verify Scylla Manager can communicate with all the Agents, and the the cluster status is OK by
   running the `sctool status` command.

📝 Scylla Monitoring Stack is based on `Prometheus` and `Grafana`. It uses connection
to `Scylla Manager`
to build structure and collect metrics. For logs it uses `Loki` and `Promtail`.

On each db node we need to configure `rsyslog` to send logs to `Loki`.

```shell
# add following to /etc/rsyslog.d/scylla.conf
if $programname == 'scylla' then @@<montoring-stack-ip>:1514;RSYSLOG_SyslogProtocol23Format
```

For now, plan is to add our app and other related services to the stack.

### Rebalancing of data after replication factor change

```cassandraql
ALTER KEYSPACE nodecosmos WITH REPLICATION = {
'class' : 'SimpleStrategy',
'replication_factor' : 3
};

```

```shell
nodetool repair nodecosmos
```
