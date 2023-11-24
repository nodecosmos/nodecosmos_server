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


Elasticsearch supports a wide range of query options that allow for precise, flexible search capabilities. Here's a list of some key query options:

1. **Match Query**: This is a standard query for full-text search. It uses a standard analyzer, so it's good for handling human-readable text.

2. **Term Query**: This is a low-level query type that looks for the exact term in the field's inverted index. It's often used for filtering or in combination with other query types.

3. **Range Query**: This allows for matching documents where a field's value falls within a specific range.

4. **Bool Query**: This allows for combining multiple queries together. It includes options for `must` (all conditions must match), `should` (at least one condition must match), and `must_not` (no conditions may match).

5. **Multi-Match Query**: This allows for running a match query on multiple fields.

6. **Phrase Match Query**: This is used for matching phrases or sequences of words. It respects the order of words.

7. **Nested Query**: This is used for querying nested objects.

8. **Wildcard and Regex Query**: These allow for matching patterns in text, using wildcard characters or regular expressions.

9. **Fuzzy Query**: This allows for matching terms that are approximately similar to the search term, to accommodate typos and small errors.

10. **Geo-Queries**: These are used for geographical data. They can include distance queries (e.g., finding all documents within a certain distance of a point), bounding box queries, and polygon queries.

11. **Aggregation**: This isn't a query, but it's a common option in Elasticsearch for summarizing and analyzing your data. You can use aggregations to calculate sum, average, min, max, etc.

12. **Highlighting**: You can use highlighting to get back exactly where and how your query matched on your documents.

Each of these queries can be combined and customized in a multitude of ways, giving Elasticsearch its powerful and flexible search capabilities. They can also be further tuned using options like boosting and fuzziness.
