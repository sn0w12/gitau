fn main() {
    if let Err(error) = contract::write_contracts() {
        panic!("failed to generate the command contract: {error}");
    }
    println!("wrote the generated command contract");
}
