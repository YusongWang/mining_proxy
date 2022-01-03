fn main() {
    foo();
}


cfg_if::cfg_if! {
    if #[cfg(feature = "agent")] {
        fn foo() { println!("foo u"); }
    } else if #[cfg(target_pointer_width = "32")] {
        fn foo() { println!("foo 32");/* non-unix, 32-bit functionality */ }
    } else {
        fn foo() { println!("foo a");/* fallback implementation */ }
    }
}
