use anyhow::{anyhow, Result};

pub fn get_bits<const START: usize, const END: usize>(n: u16) -> u16 {
    assert!(
        END <= 15 && START <= END,
        "start and end bits out of bounds"
    );
    let mask = u16::MAX >> (15 - (END - START));
    (n >> START) & mask
}

pub fn sign_extend<const NUM_BITS: usize>(n: i16) -> i16 {
    assert!(NUM_BITS <= 16);
    (n << (16 - NUM_BITS)) >> (16 - NUM_BITS)
}

pub fn truncate<const BIT_WIDTH: usize>(n: u16) -> Result<u16> {
    let extended = sign_extend::<BIT_WIDTH>(n as i16);
    if extended != n as i16 {
        return Err(anyhow!("{n} does not fit in {BIT_WIDTH} bits"));
    }
    return Ok(n & !(u16::MAX << BIT_WIDTH));
}
