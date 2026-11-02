/// `Value` of an orb is a short: a clump above 32767 is saved as `Value` x `Count` (the product the
/// pickup gives is the same), with the largest `Value` that divides it.
pub fn split_value(total: u32) -> (u32, u32) {
    const SHORT: u32 = i16::MAX as u32;
    if total <= SHORT {
        return (total, 1);
    }
    let mut value = SHORT;
    while total % value != 0 {
        value -= 1;
    }
    (value, total / value)
}
