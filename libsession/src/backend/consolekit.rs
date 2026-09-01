pub struct ConsoleKit {}

impl ConsoleKit {
    pub fn try_connect() -> Option<Self> {
        None
    }
}

impl super::Backend for ConsoleKit {}
