fn main() {
    let a:[u8; 11]= [255, 244, 255, 253, 6, 255, 244, 255, 253, 6, 13];

    println!("{}", String::from_utf8(a.to_vec()).unwrap());
}