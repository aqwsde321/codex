use std::io::Cursor;
use std::io::Read;

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn body_recorder_copies_bytes_while_reading() {
    let recorder = BodyRecorder::new();
    let mut reader = recorder.wrap(Cursor::new(b"abcdef".to_vec()));
    let mut output = String::new();
    reader
        .read_to_string(&mut output)
        .expect("read recorded body");

    assert_eq!(output, "abcdef");
    assert_eq!(recorder.bytes(), b"abcdef".to_vec());
}
