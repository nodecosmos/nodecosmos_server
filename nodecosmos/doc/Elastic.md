### Sample requests:

Get nodes where description contains "test":

```bash
curl -X GET "http://localhost:9200/nodes/_search" -H 'Content-Type: application/json' -d '
{
  "query": {
    "match": {
      "description": "test"
    }
  }
}'
```

