pub fn info(msg: impl Into<String>) {
    println!("{} {}","=".repeat(15), msg.into())
}