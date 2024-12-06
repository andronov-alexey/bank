use crate::output_record::OutputRecord;
use std::io::Write;

pub struct OutputWriter {}

impl OutputWriter {
    pub fn new() -> OutputWriter {
        Self {}
    }

    pub fn write<W: Write>(
        &self,
        writer: W,
        records: impl Iterator<Item = OutputRecord>,
    ) -> anyhow::Result<()> {
        let mut wtr = csv::Writer::from_writer(writer);
        for record in records {
            wtr.serialize(record)?;
        }
        wtr.flush()?;
        Ok(())
    }
}
