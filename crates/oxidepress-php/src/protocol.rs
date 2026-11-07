// Modified by Vabian124 on 2026-11-07 to run WordPress plugins in a PHP worker.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Length-prefixed JSON frames shared with `php/worker.php`.
//!
//! A frame is a little-endian `u32` byte length followed by a JSON object.
//! While a request is open, the worker may send `{"call": ...}` frames; Rust
//! answers with `{"result": ...}` on the same id before reading the final
//! `{"ok": ...}` frame.

use std::io::{Read, Write};

use serde_json::Value;

use crate::PhpError;

/// Eight mebibytes. Large enough for echoed plugin output, small enough to
/// reject a corrupt length before allocating it.
const MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

pub(crate) fn write_frame(writer: &mut dyn Write, value: &Value) -> Result<(), PhpError> {
    let body = serde_json::to_vec(value).map_err(|err| PhpError::Protocol(err.to_string()))?;
    let len = u32::try_from(body.len())
        .map_err(|_| PhpError::Protocol("frame is larger than u32::MAX".to_string()))?;
    if body.len() > MAX_FRAME_LEN {
        return Err(PhpError::Protocol(format!(
            "frame length {} is not allowed",
            body.len()
        )));
    }
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read_frame(reader: &mut dyn Read) -> Result<Value, PhpError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_LEN {
        return Err(PhpError::Protocol(format!(
            "frame length {len} is not allowed"
        )));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|err| PhpError::Protocol(err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;

    #[test]
    fn frame_round_trips() {
        let value = json!({"op": "load", "path": "a.php", "nested": [1, "x"]});
        let mut buf = Vec::new();
        write_frame(&mut buf, &value).unwrap();

        let mut cursor = Cursor::new(buf);
        assert_eq!(read_frame(&mut cursor).unwrap(), value);
        assert!(read_frame(&mut cursor).is_err());
    }

    #[test]
    fn two_frames_read_in_order() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &json!({"id": 1})).unwrap();
        write_frame(&mut buf, &json!({"id": 2})).unwrap();
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_frame(&mut cursor).unwrap()["id"], 1);
        assert_eq!(read_frame(&mut cursor).unwrap()["id"], 2);
    }

    #[test]
    fn truncated_frame_is_an_error() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_le_bytes());
        assert!(read_frame(&mut Cursor::new(buf)).is_err());
    }

    #[test]
    fn huge_length_is_rejected_without_reading_a_body() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&u32::MAX.to_le_bytes());
        let err = read_frame(&mut Cursor::new(buf)).unwrap_err();
        assert!(err.to_string().contains("not allowed"), "{err}");
    }
}
