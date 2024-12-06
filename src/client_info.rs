#[derive(Debug)]
pub struct ClientInfo {
    pub on_hold: f64,
    pub available: f64,
    pub is_locked: bool,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            on_hold: 0f64,
            available: 0f64,
            is_locked: false,
        }
    }
}
