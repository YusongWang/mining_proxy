use proxy::util::get_wallet;

fn main() {
    let a= "0x98be5c44d574b96b320dffb0ccff116bda433b8e";
    extern crate short_crypt;
    use short_crypt::ShortCrypt;
    let sc = ShortCrypt::new(a);
    
    let a = get_wallet();
    println!("{}",a);

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
