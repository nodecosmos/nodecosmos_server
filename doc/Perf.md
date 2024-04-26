### Elapsed time:

```rust 
    let now = std::time::Instant::now();

    for post in posts {
        post.unwrap();
    }

    println!("elapsed: {:?}", now.elapsed());
```

### Login with curl

    curl -c curl -c cookies.txt -H "Content-Type: application/json" -d '{"username_or_email":"goran","password":"gorangoran"}' http://localhost:3000/sessions/login

### Create 1000 nodes

    seq 1 1000 | xargs -n1 -P10 -I{} curl -b cookies.txt -H "Content-Type: application/json" -d '{"parentId":"4f9543c6-a911-4861-b4ee-c76751e45d02","title":"cc 1", "isPublic": true, "isRoot": false, "order": 1}' http://localhost:3000/nodes

### Check if logged in

    curl -b cookies.txt http://localhost:3000/sessions/sync 