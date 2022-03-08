{
use ::static_files::resource::new_resource as n;
use ::std::include_bytes as i;
let mut r = ::std::collections::HashMap::new();
r.insert("file2.txt",n(i!("/Users/yusong/.cargo/git/checkouts/actix-web-static-files-56e2c41287c05b1a/19fd166/tests/file2.txt"),1645417586,"text/plain"));
r.insert("file1.txt",n(i!("/Users/yusong/.cargo/git/checkouts/actix-web-static-files-56e2c41287c05b1a/19fd166/tests/file1.txt"),1645417586,"text/plain"));
r.insert("index.html",n(i!("/Users/yusong/.cargo/git/checkouts/actix-web-static-files-56e2c41287c05b1a/19fd166/tests/index.html"),1645417586,"text/html"));
r.insert("file3.info",n(i!("/Users/yusong/.cargo/git/checkouts/actix-web-static-files-56e2c41287c05b1a/19fd166/tests/file3.info"),1645417586,"application/octet-stream"));
r
}
