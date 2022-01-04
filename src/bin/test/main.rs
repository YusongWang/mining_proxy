fn main() {
    let mut p = std::process::Command::new("target/debug/proxy");
    //p.env();
    let mut a = p.spawn().unwrap();
    // a.stdout

    a.wait().unwrap();
}


// cfg_if::cfg_if! {
//     if #[cfg(feature = "agent")] {
//         fn foo() { println!("foo u"); }
//     } else if #[cfg(target_pointer_width = "32")] {
//         fn foo() { println!("foo 32");/* non-unix, 32-bit functionality */ }
//     } else {
//         fn foo() { println!("foo a");/* fallback implementation */ }
//     }
// }
