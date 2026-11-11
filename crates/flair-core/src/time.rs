use std::ops::Add;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct Millis(pub u64);

impl Millis {
    pub fn saturating_sub(self, rhs: Millis) -> Millis {
        Millis(self.0.saturating_sub(rhs.0))
    }
}

impl Add for Millis {
    type Output = Millis;

    fn add(self, rhs: Millis) -> Millis {
        Millis(self.0.saturating_add(rhs.0))
    }
}
