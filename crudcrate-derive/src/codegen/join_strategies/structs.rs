/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub struct JoinConfig {
    pub on_one: bool,
    pub on_all: bool,
    pub depth: Option<u8>,
    pub relation: Option<String>,
    pub path: Option<String>,
}

impl JoinConfig {
    /// Check if recursion is unlimited (no explicit depth set)
    pub fn is_unlimited_recursion(&self) -> bool {
        self.depth.is_none()
    }
}
