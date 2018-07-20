extern crate msg_transmitter;

use msg_transmitter::*;

#[test]
fn test_vecu8_u64() {
    for i in 0..(1 << 32) as u64 {
        assert_eq!(i, four_vecu8_to_number(number_to_four_vecu8(i)));
    }
}