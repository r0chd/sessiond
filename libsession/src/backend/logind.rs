pub struct Logind {}

impl Logind {
    pub fn try_connect() -> Option<Self> {
        None
    }
}

impl super::Backend for Logind {}
