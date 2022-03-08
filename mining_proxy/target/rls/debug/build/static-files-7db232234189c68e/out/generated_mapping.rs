{
use ::static_files::resource::new_resource as n;
use ::std::include_bytes as i;
let mut r = ::std::collections::HashMap::new();
r.insert("file2.txt",n(i!("/Users/yusong/.cargo/registry/src/mirrors.ustc.edu.cn-61ef6e0cd06fb9b8/static-files-0.2.3/tests/file2.txt"),1,"text/plain"));
r.insert("file1.txt",n(i!("/Users/yusong/.cargo/registry/src/mirrors.ustc.edu.cn-61ef6e0cd06fb9b8/static-files-0.2.3/tests/file1.txt"),1,"text/plain"));
r.insert("index.html",n(i!("/Users/yusong/.cargo/registry/src/mirrors.ustc.edu.cn-61ef6e0cd06fb9b8/static-files-0.2.3/tests/index.html"),1,"text/html"));
r.insert("file3.info",n(i!("/Users/yusong/.cargo/registry/src/mirrors.ustc.edu.cn-61ef6e0cd06fb9b8/static-files-0.2.3/tests/file3.info"),1,"application/octet-stream"));
r
}
