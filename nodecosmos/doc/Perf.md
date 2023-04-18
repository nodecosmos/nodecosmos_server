### Elapsed time:

```rust 
    let now = std::time::Instant::now();

    for post in posts {
        post.unwrap();
    }

    println!("elapsed: {:?}", now.elapsed());
```
