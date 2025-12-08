// Example usage of preflop_ranges module
// Run with: cargo run --example preflop_demo

use pluely::poker::{get_preflop_action, Action};

fn main() {
    println!("=== GTO Preflop Opening Ranges Demo ===\n");

    // Test various hands from BTN
    println!("Button (BTN) Opening Range:");
    test_hand("AA", "BTN");
    test_hand("AKs", "BTN");
    test_hand("22", "BTN");
    test_hand("54s", "BTN");
    test_hand("ATo", "BTN");
    test_hand("72o", "BTN");

    println!("\nCutoff (CO) Opening Range:");
    test_hand("AA", "CO");
    test_hand("22", "CO");
    test_hand("K5s", "CO");
    test_hand("K4s", "CO");

    println!("\nEarly Position (EP) Opening Range:");
    test_hand("AA", "EP");
    test_hand("66", "EP");
    test_hand("55", "EP");
    test_hand("AJs", "EP");
    test_hand("AQo", "EP");
    test_hand("AJo", "EP");

    println!("\nMiddle Position (MP) Opening Range:");
    test_hand("55", "MP");
    test_hand("44", "MP");
    test_hand("A5s", "MP");

    // Test with card notation (e.g., "Ah Kh")
    println!("\nCard Notation Tests:");
    test_hand("Ah Kh", "BTN");  // AKs
    test_hand("As Kd", "BTN");  // AKo
    test_hand("9c 9d", "BTN");  // 99
}

fn test_hand(hand: &str, position: &str) {
    match get_preflop_action(hand, position) {
        Some(Action::Raise(amount)) => {
            println!("  {} from {}: RAISE {}bb ✓", hand, position, amount);
        }
        Some(Action::Fold) => {
            println!("  {} from {}: FOLD ✗", hand, position);
        }
        Some(Action::Call) => {
            println!("  {} from {}: CALL", hand, position);
        }
        Some(Action::Check) => {
            println!("  {} from {}: CHECK", hand, position);
        }
        None => {
            println!("  {} from {}: Invalid hand or position", hand, position);
        }
    }
}
