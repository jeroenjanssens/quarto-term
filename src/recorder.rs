use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

enum Format {
    Asciicast,
    Termshow,
}

pub struct Recorder {
    writer: BufWriter<File>,
    start: Instant,
    last_event: Instant,
    format: Format,
}

impl Recorder {
    pub fn new(path: &str, cols: u16, rows: u16) -> std::io::Result<Self> {
        let format = if path.ends_with(".termshow") {
            Format::Termshow
        } else {
            Format::Asciicast
        };

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        match &format {
            Format::Asciicast => {
                let header = serde_json::json!({
                    "version": 2,
                    "width": cols,
                    "height": rows,
                });
                writeln!(writer, "{}", header)?;
            }
            Format::Termshow => {
                let header = serde_json::json!({
                    "version": 1,
                    "format": "termshow",
                    "term": { "cols": cols, "rows": rows },
                });
                writeln!(writer, "{}", header)?;

                // Generate companion .termshow.yml
                let yml_path = format!("{}.yml", path);
                if let Ok(mut yml) = File::create(&yml_path) {
                    let basename = std::path::Path::new(path)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(path);
                    let _ = write!(yml, "source: {}\n", basename);
                }
            }
        }
        writer.flush()?;

        let now = Instant::now();
        Ok(Self {
            writer,
            start: now,
            last_event: now,
            format,
        })
    }

    pub fn record_output(&mut self, bytes: &[u8]) {
        self.record_event("o", bytes);
    }

    pub fn record_input(&mut self, _bytes: &[u8]) {
        // Input events not written for asciicast (asciinema 3.x hangs).
        // For termshow, input events could be useful but we skip them
        // for now since our typing is synthetic.
    }

    pub fn finish(&mut self) {
        let _ = self.writer.flush();
    }

    fn record_event(&mut self, event_type: &str, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let escaped = serde_json::to_string(text.as_ref()).unwrap_or_default();

        match self.format {
            Format::Asciicast => {
                let elapsed = self.start.elapsed().as_secs_f64();
                let _ = writeln!(self.writer, "[{:.6}, \"{}\", {}]", elapsed, event_type, escaped);
            }
            Format::Termshow => {
                let now = Instant::now();
                let delay = now.duration_since(self.last_event).as_secs_f64();
                self.last_event = now;
                let _ = writeln!(self.writer, "[{:.6}, \"{}\", {}]", delay, event_type, escaped);
            }
        }
        let _ = self.writer.flush();
    }
}
