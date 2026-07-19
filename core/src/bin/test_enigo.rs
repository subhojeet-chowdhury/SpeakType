use enigo::{Enigo, Key, Keyboard, Settings, Direction};
fn main() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    enigo.key(Key::Meta, Direction::Press).unwrap();
    // In enigo 0.2, it might be Key::V or similar. Let's try Key::V
    // enigo.key(Key::V, Direction::Click).unwrap();
}
