#[derive(Debug, Clone)]
pub struct BbCodeOptions {
    pub max_depth: usize,
    pub max_tags: usize,
    pub max_input_size: usize,
}

impl Default for BbCodeOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_tags: 500,
            max_input_size: 50 * 1024,
        }
    }
}
