use diesel::table;

table! {
    pools (id) {
        id -> Integer,
        name -> Text,
        tcp_port -> Integer,
        encrypt_port ->Integer,
        ssl_port -> Integer,
        pool_tcp_address -> Text,
        pool_ssl_address -> Text,
        share_tcp_address -> Text,
        share_rate -> Float4,
        share_wallet  ->Text,
        share_name  ->Text,
        share  ->Integer,
        share_alg  ->Integer,
        p12_path  ->Text,
        p12_pass  ->Text,
        key  ->Text,
        iv  ->Text,
        coin  -> Text,
        is_online -> Integer,
        is_open ->Integer,
        pid ->Integer,
    }
}
