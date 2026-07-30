use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

pub struct Recorder {
    writer: BufWriter<File>,
    start: Instant,
}

impl Recorder {
    pub fn new(path: &str, cols: u16, rows: u16) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let header = serde_json::json!({
            "version": 2,
            "width": cols,
            "height": rows,
        });
        writeln!(writer, "{}", header)?;
        writer.flush()?;

        Ok(Self {
            writer,
            start: Instant::now(),
        })
    }

    pub fn record_output(&mut self, bytes: &[u8]) {
        self.record_event("o", bytes);
    }

    pub fn record_input(&mut self, bytes: &[u8]) {
        self.record_event("i", bytes);
    }

    fn record_event(&mut self, event_type: &str, bytes: &[u8]) {
        let elapsed = self.start.elapsed().as_secs_f64();
        let text = String::from_utf8_lossy(bytes);
        let escaped = serde_json::to_string(text.as_ref()).unwrap_or_default();
        let _ = writeln!(self.writer, "[{:.6}, \"{}\", {}]", elapsed, event_type, escaped);
        let _ = self.writer.flush();
    }
}
