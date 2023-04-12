
#![feature(test)]

extern crate test;

pub fn add_two(a: i32) -> i32 {
    a + 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[tokio::main]
    #[bench]
    async fn bench_charybdis(b: &mut Bencher) {
        let session = init_session().await;

        for _ in 0..100000 {
            let user = User {
                id: Uuid::new_v4(),
                username: "charybdis".to_string(),
                password: "charybdis".to_string(),
                hashed_password: "charybdis".to_string(),
                email: "charybdis@charybdis.com".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            user.insert(&session).await.unwrap_or_else(|e| {
                panic!("Error inserting user: {}", e);
            });
        };
    }

    // #[tokio::main]
    // #[bench]
    // async fn bench_plain_query(b: &mut Bencher) {
    //     let session = init_session().await;
    //
    //     for _ in 0..100000 {
    //        let values = (Uuid::new_v4(), "charybdis".to_string(), "charybdis".to_string(), "charybdis".to_string(), "charybdis@charybdis.com".to_string(), Utc::now(), Utc::now());
    //         let query = "INSERT INTO users (id, username, password, hashed_password, email, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)";
    //         session.execute(query, values).await.unwrap_or_else(|e| {
    //             panic!("Error inserting user: {}", e);
    //         });
    //     };
    // }
}
