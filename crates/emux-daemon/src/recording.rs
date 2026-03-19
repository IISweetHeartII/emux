//! Session recording and replay in asciicast v2 format.
//!
//! The [asciicast v2 format](https://docs.asciinema.org/manual/asciicast/v2/)
//! stores terminal sessions as newline-delimited JSON:
//!
//! - Line 1: a header object with `version`, `width`, `height`, and `timestamp`.
//! - Subsequent lines: event tuples `[time, type, data]` where `time` is seconds
//!   since the recording started, `type` is `"o"` (output), `"i"` (input), or
//!   `"r"` (resize), and `data` is the payload string.

use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Header for asciicast v2 format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingHeader {
    /// Format version (always 2).
    pub version: u8,
    /// Terminal width in columns.
    pub width: u16,
    /// Terminal height in rows.
    pub height: u16,
    /// Unix timestamp when recording started.
    pub timestamp: u64,
    /// Optional title for the recording.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Errors that can occur during recording or replay.
#[derive(Debug)]
pub enum RecordingError {
    /// An I/O error occurred.
    Io(std::io::Error),
    /// A serialization or deserialization error occurred.
    Serialize(String),
}

impl fmt::Display for RecordingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordingError::Io(e) => write!(f, "recording I/O error: {e}"),
            RecordingError::Serialize(msg) => write!(f, "recording serialization error: {msg}"),
        }
    }
}

impl std::error::Error for RecordingError {}

impl From<std::io::Error> for RecordingError {
    fn from(e: std::io::Error) -> Self {
        RecordingError::Io(e)
    }
}

impl From<serde_json::Error> for RecordingError {
    fn from(e: serde_json::Error) -> Self {
        RecordingError::Serialize(e.to_string())
    }
}

/// A session recorder that writes asciicast v2 format.
pub struct SessionRecorder {
    writer: BufWriter<File>,
    path: PathBuf,
    start_time: Instant,
    started: bool,
}

impl SessionRecorder {
    /// Create a new recorder writing to `path`.
    ///
    /// The header is written immediately. Subsequent calls to `record_output`,
    /// `record_input`, and `record_resize` append event lines.
    pub fn new(
        path: &Path,
        width: u16,
        height: u16,
        title: Option<String>,
    ) -> Result<Self, RecordingError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let header = RecordingHeader {
            version: 2,
            width,
            height,
            timestamp,
            title,
        };

        let header_json = serde_json::to_string(&header)?;
        writeln!(writer, "{header_json}")?;

        Ok(Self {
            writer,
            path: path.to_path_buf(),
            start_time: Instant::now(),
            started: true,
        })
    }

    /// Record a terminal output event.
    pub fn record_output(&mut self, data: &[u8]) -> Result<(), RecordingError> {
        self.write_event("o", &String::from_utf8_lossy(data))
    }

    /// Record a terminal input event.
    pub fn record_input(&mut self, data: &[u8]) -> Result<(), RecordingError> {
        self.write_event("i", &String::from_utf8_lossy(data))
    }

    /// Record a terminal resize event.
    pub fn record_resize(&mut self, cols: u16, rows: u16) -> Result<(), RecordingError> {
        self.write_event("r", &format!("{cols}x{rows}"))
    }

    /// Seconds elapsed since the recording started.
    pub fn elapsed(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Flush and close the recording, returning the file path.
    pub fn finish(mut self) -> Result<PathBuf, RecordingError> {
        self.writer.flush()?;
        self.started = false;
        Ok(self.path.clone())
    }

    /// Write a single event line: `[time, type, data]`.
    fn write_event(&mut self, event_type: &str, data: &str) -> Result<(), RecordingError> {
        let elapsed = self.elapsed();
        let event = serde_json::to_string(&(elapsed, event_type, data))?;
        writeln!(self.writer, "{event}")?;
        Ok(())
    }
}

impl Drop for SessionRecorder {
    fn drop(&mut self) {
        if self.started {
            let _ = self.writer.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// Recording reader (replay)
// ---------------------------------------------------------------------------

/// The type of a recorded event.
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    /// Terminal output (`"o"`).
    Output,
    /// Terminal input (`"i"`).
    Input,
    /// Terminal resize (`"r"`).
    Resize,
}

/// A single event parsed from a recording file.
#[derive(Debug, Clone)]
pub struct RecordingEvent {
    /// Seconds since the recording started.
    pub time: f64,
    /// The kind of event.
    pub event_type: EventType,
    /// The event payload.
    pub data: String,
}

/// Reader for recorded asciicast v2 sessions.
pub struct RecordingReader {
    header: RecordingHeader,
    events: Vec<RecordingEvent>,
}

impl RecordingReader {
    /// Open and parse a recording file.
    pub fn open(path: &Path) -> Result<Self, RecordingError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // First line is the header.
        let header_line = lines
            .next()
            .ok_or_else(|| RecordingError::Serialize("empty recording file".into()))??;
        let header: RecordingHeader = serde_json::from_str(&header_line)?;

        // Remaining lines are events.
        let mut events = Vec::new();
        for line_result in lines {
            let line = line_result?;
            if line.trim().is_empty() {
                continue;
            }
            let tuple: (f64, String, String) = serde_json::from_str(&line)?;
            let event_type = match tuple.1.as_str() {
                "o" => EventType::Output,
                "i" => EventType::Input,
                "r" => EventType::Resize,
                other => {
                    return Err(RecordingError::Serialize(format!(
                        "unknown event type: {other}"
                    )));
                }
            };
            events.push(RecordingEvent {
                time: tuple.0,
                event_type,
                data: tuple.2,
            });
        }

        Ok(Self { header, events })
    }

    /// The recording header.
    pub fn header(&self) -> &RecordingHeader {
        &self.header
    }

    /// All parsed events.
    pub fn events(&self) -> &[RecordingEvent] {
        &self.events
    }

    /// Total duration in seconds (time of the last event, or 0.0 if empty).
    pub fn duration(&self) -> f64 {
        self.events.last().map_or(0.0, |e| e.time)
    }

    /// Number of events in the recording.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_header_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("header_format.cast");
        let rec = SessionRecorder::new(&path, 80, 24, Some("test".into())).unwrap();
        rec.finish().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let first_line = contents.lines().next().unwrap();
        let header: serde_json::Value = serde_json::from_str(first_line).unwrap();

        assert_eq!(header["version"], 2);
        assert_eq!(header["width"], 80);
        assert_eq!(header["height"], 24);
        assert_eq!(header["title"], "test");
        assert!(header["timestamp"].as_u64().unwrap() > 0);
    }

    #[test]
    fn recording_output_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output_event.cast");
        let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
        rec.record_output(b"hello world").unwrap();
        rec.finish().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let event_line = contents.lines().nth(1).unwrap();
        let event: (f64, String, String) = serde_json::from_str(event_line).unwrap();

        assert!(event.0 >= 0.0);
        assert_eq!(event.1, "o");
        assert_eq!(event.2, "hello world");
    }

    #[test]
    fn recording_input_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input_event.cast");
        let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
        rec.record_input(b"ls -la\n").unwrap();
        rec.finish().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let event_line = contents.lines().nth(1).unwrap();
        let event: (f64, String, String) = serde_json::from_str(event_line).unwrap();

        assert!(event.0 >= 0.0);
        assert_eq!(event.1, "i");
        assert_eq!(event.2, "ls -la\n");
    }

    #[test]
    fn recording_resize_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resize_event.cast");
        let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
        rec.record_resize(120, 40).unwrap();
        rec.finish().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let event_line = contents.lines().nth(1).unwrap();
        let event: (f64, String, String) = serde_json::from_str(event_line).unwrap();

        assert_eq!(event.1, "r");
        assert_eq!(event.2, "120x40");
    }

    #[test]
    fn recording_timing_increases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("timing_increases.cast");
        let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
        rec.record_output(b"first").unwrap();
        // Small busy-wait to ensure time advances.
        std::thread::sleep(std::time::Duration::from_millis(10));
        rec.record_output(b"second").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        rec.record_output(b"third").unwrap();
        rec.finish().unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let mut times: Vec<f64> = Vec::new();
        for line in contents.lines().skip(1) {
            let event: (f64, String, String) = serde_json::from_str(line).unwrap();
            times.push(event.0);
        }

        assert_eq!(times.len(), 3);
        assert!(times[1] >= times[0], "second event should be >= first");
        assert!(times[2] >= times[1], "third event should be >= second");
    }

    #[test]
    fn recording_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roundtrip.cast");
        {
            let mut rec = SessionRecorder::new(&path, 132, 43, Some("roundtrip".into())).unwrap();
            rec.record_output(b"hello").unwrap();
            rec.record_input(b"world").unwrap();
            rec.record_resize(100, 50).unwrap();
            rec.finish().unwrap();
        }

        let reader = RecordingReader::open(&path).unwrap();
        assert_eq!(reader.header().version, 2);
        assert_eq!(reader.header().width, 132);
        assert_eq!(reader.header().height, 43);
        assert_eq!(reader.header().title.as_deref(), Some("roundtrip"));

        let events = reader.events();
        assert_eq!(events.len(), 3);

        assert_eq!(events[0].event_type, EventType::Output);
        assert_eq!(events[0].data, "hello");

        assert_eq!(events[1].event_type, EventType::Input);
        assert_eq!(events[1].data, "world");

        assert_eq!(events[2].event_type, EventType::Resize);
        assert_eq!(events[2].data, "100x50");
    }

    #[test]
    fn recording_reader_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("duration.cast");
        {
            let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
            rec.record_output(b"a").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            rec.record_output(b"b").unwrap();
            rec.finish().unwrap();
        }

        let reader = RecordingReader::open(&path).unwrap();
        let dur = reader.duration();
        assert!(dur >= 0.04, "duration {dur} should be >= 0.04s");
        // Duration should equal the last event's time.
        assert_eq!(dur, reader.events().last().unwrap().time);
    }

    #[test]
    fn recording_reader_event_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event_count.cast");
        {
            let mut rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
            rec.record_output(b"one").unwrap();
            rec.record_output(b"two").unwrap();
            rec.record_input(b"three").unwrap();
            rec.record_resize(90, 30).unwrap();
            rec.record_output(b"four").unwrap();
            rec.finish().unwrap();
        }

        let reader = RecordingReader::open(&path).unwrap();
        assert_eq!(reader.event_count(), 5);
    }

    #[test]
    fn recording_empty_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_session.cast");
        {
            let rec = SessionRecorder::new(&path, 80, 24, None).unwrap();
            rec.finish().unwrap();
        }

        let reader = RecordingReader::open(&path).unwrap();
        assert_eq!(reader.header().version, 2);
        assert_eq!(reader.event_count(), 0);
        assert_eq!(reader.duration(), 0.0);
    }
}
