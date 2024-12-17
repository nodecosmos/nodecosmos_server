pub const MAX_PARALLEL_REQUESTS: usize = 25;
pub const BATCH_CHUNK_SIZE: usize = 100;
pub const PAGE_SIZE: i32 = 51;

/// `max_partition_key_restrictions_per_query` is the maximum number of distinct partition key restrictions per query.
/// `max_clustering_key_restrictions_per_query` is the maximum number of distinct clustering key restrictions per query.
/// This limit places a bound on the size of IN tuples, especially when multiple clustering key columns have IN
/// restrictions. Increasing this value can result in server instability.
pub const MAX_WHERE_IN_CHUNK_SIZE: usize = 100;

pub const BLACKLIST_USERNAMES: [&str; 26] = [
    "404",
    "about",
    "admin",
    "administrator",
    "auth",
    "blacklist",
    "contact",
    "contact-us",
    "doc",
    "home",
    "invitations",
    "login",
    "mine",
    "node",
    "nodecosmos",
    "nodes",
    "nodescosmos",
    "password",
    "reset_password",
    "social",
    "socials",
    "support",
    "test",
    "tester",
    "testing",
    "username",
];
