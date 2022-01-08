fn main() {
    let a = "0x98be5c44d574b96b320dffb0ccff116bda433b8e";
    extern crate short_crypt;
    use short_crypt::ShortCrypt;
    let _sc = ShortCrypt::new(a);

    // println!(
    //     "{}",
    //     sc.encrypt(("hellowol".len(), "hellowol".as_bytes().to_vec()))
    // );
}
