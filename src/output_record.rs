use serde::{Deserialize, Serialize, Serializer};

pub trait OutputRecordProvider {
    fn get_records(&self) -> impl Iterator<Item = OutputRecord>;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OutputRecord {
    pub client: u64,
    #[serde(serialize_with = "fixed_width")]
    pub available: f64,
    #[serde(serialize_with = "fixed_width")]
    pub held: f64,
    #[serde(serialize_with = "fixed_width")]
    pub total: f64,
    pub locked: bool,
}

fn fixed_width<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{}", trim_trailing_zeros(*value)))
}

fn trim_trailing_zeros(value: f64) -> f64 {
    // improvement: replace magic number with constant
    (value * 10_000.0).round() / 10_000.0
}
