use crate::MessageNumberType;

#[derive(Default)]
pub struct MsgNumGenerator {
    next: MessageNumberType,
}

impl MsgNumGenerator {
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> MessageNumberType {
        let next = self.next + 1;
        std::mem::replace(&mut self.next, next)
    }

    pub fn new() -> Self {
        Self::default()
    }
}
