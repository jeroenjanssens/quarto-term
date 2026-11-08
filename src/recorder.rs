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

    /// Record terminal output. The caller decodes the PTY bytes, so that a
    /// multibyte character split between chunks isn't garbled.
    pub fn record_output(&mut self, text: &str) {
        self.record_event("o", text);
    }

    pub fn finish(&mut self) {
        let _ = self.writer.flush();
    }

    fn record_event(&mut self, event_type: &str, text: &str) {
        let escaped = serde_json::to_string(text).unwrap_or_default();

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("quarto-term-recorder-{}-{name}", std::process::id()))
    }

    /// The output events ("o") of a recording, in order
    fn output_events(path: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .skip(1)
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|event| event[1] == "o")
            .map(|event| event[2].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn records_multibyte_text_as_is() {
        let path = temp_path("multibyte.cast");
        let mut rec = Recorder::new(path.to_str().unwrap(), 80, 24).unwrap();
        rec.record_output("┌─┐ ✓é😀\r\n");
        rec.finish();
        drop(rec);
        assert_eq!(output_events(&path), vec!["┌─┐ ✓é😀\r\n"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn records_escape_sequences_in_termshow() {
        let path = temp_path("escapes.termshow");
        let mut rec = Recorder::new(path.to_str().unwrap(), 80, 24).unwrap();
        rec.record_output("\x1b[32m─\x1b[0m");
        rec.record_output("\"quoted\"");
        rec.finish();
        drop(rec);
        assert_eq!(output_events(&path), vec!["\x1b[32m─\x1b[0m", "\"quoted\""]);
        std::fs::remove_file(&path).ok();
    }
}
