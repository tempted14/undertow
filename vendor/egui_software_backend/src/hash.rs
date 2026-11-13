/// Ordered 64-bit mesh fingerprint; one final avalanche replaces per-field mixing.
pub struct Hash64(pub u64);

impl Hash64 {
    #[inline(always)]
    pub fn new_fnv() -> Self {
        Hash64(0xcbf29ce484222325)
    }

    #[inline(always)]
    pub fn hash_wrap(&mut self, v: u32) {
        self.0 = (self.0 ^ v as u64).wrapping_mul(0x100000001b3);
    }

    pub fn finalize(&self) -> u64 {
        let mut x = self.0;
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
        x ^ (x >> 33)
    }
}
