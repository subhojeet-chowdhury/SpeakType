use global_hotkey::hotkey::Code;
use std::str::FromStr;
fn main() {
    println!("Space: {:?}", Code::from_str("Space"));
    println!("SPACE: {:?}", Code::from_str("SPACE"));
}
