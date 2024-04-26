Building a version control system similar to Git for your database models in Scylla requires careful
planning and design. The goal is to efficiently store and track changes to Node, NodeDescendants,
Workflow, Flow, FlowStep, and InputOutput entities, along with their attributes title and
description. Let's break down the requirements and propose a design strategy.

1. ### Directed Acyclic Graph (DAG) for Version Control
   Storing Versions: Each entity (Node, Workflow, etc.) should have a version identifier (e.g., a
   hash or a sequential ID). Whenever an entity is modified, a new version with a new ID is created.
   Parent-Child Relationships: Store the parent version ID with each version to establish a DAG.
   This allows tracking the history of changes.
   Efficient Storage: Utilize lightweight deltas instead of full copies for minor changes. Store
   only the differences from the previous version.
2. ### Efficient Data Storage
   Mirror Tables with Compression: For deleted records, move them to mirror tables with higher
   compression ratios (like DEFLATE). This strategy balances the need for historical data retention
   with storage efficiency.
   Data Deletion and Archiving: When a record is deleted from the main table, archive it in the
   mirror table with a flag indicating its deletion status.
3. ### Tracking History and Checkouts
   Historical Snapshots: Maintain a table to store snapshots of entity states at specific points in
   time. This table should reference the version IDs of all entities at that snapshot.
   Fast Retrieval for Checkouts: To quickly rebuild a specific version, use the snapshot table to
   fetch the relevant versions of each entity.
4. ### Branching and Contribution Requests
   Branches: Implement branching by associating a branch identifier with each version. When a new
   branch is created, it initially points to the same version as its parent branch.
   Handling Contribution Requests: Similar to pull requests, contribution requests can merge changes
   from one branch to another. This requires a mechanism to compare versions and resolve conflicts.
5. ### Implementing the Model
   Schema Design: Your schema should include tables for storing the current state of each entity,
   the historical versions (with parent-child relationships), the snapshots, and the mirror tables
   for deleted records.
   Metadata Tracking: Each record should include metadata such as version ID, branch ID,
   creation/modification date, and a flag for deletion status.
6. ### Application Logic
   Version Creation: Implement logic in your application to handle the creation of new versions upon
   modifications, and the archiving of deleted records.
   Conflict Resolution: Develop algorithms for detecting and resolving conflicts during merges in
   contribution requests.
7. ### Performance Considerations
   Indexing: Proper indexing is crucial for efficient retrieval of versions and historical data.
   Background Processes: Consider using background jobs for resource-intensive tasks like archiving
   deleted records or rebuilding specific versions for checkout.