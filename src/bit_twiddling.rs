pub fn get_bits<const START: usize, const END: usize>(n: u16) -> u16 {
    assert!(END <= 15 && START <= END, "start and end bits out of bounds");
    let mask = u16::MAX >> (15 - (END - START));
    (n >> START) & mask
}

pub fn sign_extend<const NUM_BITS: usize>(n: i16) -> i16 {
    assert!(NUM_BITS <= 16);
    (n << (16 - NUM_BITS)) >> (16 - NUM_BITS)
}
