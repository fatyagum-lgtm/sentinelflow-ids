use crate::event::Event;
use std::io::BufRead;

/// Bounded per-line allocation. Oversized lines are drained to the next newline.
pub struct JsonLines<R> {
    reader: R,
    limit: usize,
    line: u64,
    ended: bool,
}
impl<R: BufRead> JsonLines<R> {
    pub fn new(reader: R, limit: usize) -> Self {
        Self {
            reader,
            limit: limit.clamp(256, 65536),
            line: 0,
            ended: false,
        }
    }
}
impl<R: BufRead> Iterator for JsonLines<R> {
    type Item = Result<Event, String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let mut bytes = Vec::new();
        let mut oversize = false;
        loop {
            let available = match self.reader.fill_buf() {
                Ok(b) => b,
                Err(_) => {
                    self.ended = true;
                    return Some(Err("Input read failed".into()));
                }
            };
            if available.is_empty() {
                self.ended = true;
                break;
            }
            let newline = available.iter().position(|b| *b == b'\n');
            let consumed = newline.map_or(available.len(), |i| i + 1);
            if bytes.len() + consumed <= self.limit && !oversize {
                bytes.extend_from_slice(&available[..consumed]);
            } else {
                oversize = true;
            }
            self.reader.consume(consumed);
            if newline.is_some() {
                break;
            }
        }
        if bytes.is_empty() && !oversize && self.ended {
            return None;
        }
        self.line = self.line.saturating_add(1);
        let prefix = format!("Line {}", self.line);
        if oversize {
            return Some(Err(format!(
                "{prefix}: record exceeds {} bytes",
                self.limit
            )));
        }
        Some(
            serde_json::from_slice::<Event>(&bytes)
                .map_err(|e| format!("{prefix}: invalid event JSON at column {}", e.column()))
                .and_then(|e| {
                    e.validate()
                        .map_err(|reason| format!("{prefix}: {reason}"))?;
                    Ok(e)
                }),
        )
    }
}
