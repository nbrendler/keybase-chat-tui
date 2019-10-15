use client::{KeybaseCommand, KeybaseMethod, keybase_exec};

fn main() {
    let result = keybase_exec(KeybaseCommand { method: KeybaseMethod::list}).unwrap();
    println!("{}", result);
}
