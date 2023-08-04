### Elapsed time:

```rust 
    let now = std::time::Instant::now();

    for post in posts {
        post.unwrap();
    }

    println!("elapsed: {:?}", now.elapsed());
```

### Login with curl
    curl -c curl -c cookies.txt -H "Content-Type: application/json" -d '{"username_or_email":"goran","password":"gorangoran"}' http://192.168.1.104:3000/sessions/login

### Create 1000 nodes
    seq 1 1000 | xargs -n1 -P10 -I{} curl -b cookies.txt -H "Content-Type: application/json" -d '{"rootId":"6e757f78-8ee1-487b-a418-f8785a5a01b7","parentId":"6e757f78-8ee1-487b-a418-f8785a5a01b7","title":"cc 1"}' http://192.168.1.104:3000/nodes
    seq 1 1000 | xargs -n1 -P10 -I{} curl -b cookies.txt -H "Content-Type: application/json" -d "{\"rootId\":\"40b52b23-b0c7-4b6f-9025-41108d29d978\",\"parentId\":\"9871bd71-afca-4c3f-a8ac-2b6a4032c102\",\"title\":\"cc {}\"}" http://192.168.1.104:3000/nodes


### Check if logged in
    curl -b cookies.txt http://192.168.1.104:3000/sessions/sync 