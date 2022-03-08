use ::static_files::resource::new_resource as n;
use ::std::include_bytes as i;

use ::static_files::Resource;
use ::std::collections::HashMap;
mod set_1;
#[allow(clippy::unreadable_literal)] pub fn generate() -> ::std::collections::HashMap<&'static str, ::static_files::Resource> {
let mut r = ::std::collections::HashMap::new();
set_1::generate(&mut r);
r
}
