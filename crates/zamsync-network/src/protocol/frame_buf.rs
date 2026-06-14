use super::frame::MAX_FRAME_SIZE;
use std::io::{ErrorKind, Read};
use zstd;
use zamsync_core::{ZamError, ZamResult};

/// Per-connection receive buffer.
///
/// The 50ms `read_timeout` on TCP sockets is used to poll multiple peers without
/// blocking forever, but it means a `read_exact` inside `read_frame` can be
/// interrupted mid-frame on very slow links (e.g. 3 KB/s). When that happens
/// the partial bytes that were already pulled from the kernel buffer are lost,
/// which shifts every subsequent frame by some number of bytes and breaks the
/// length-prefix framing entirely.
///
/// `FrameBuffer` fixes this by accumulating all received bytes in `buf` and
/// only returning a complete frame once enough bytes are present. Partial reads
/// due to timeout just leave bytes in the buffer for the next poll cycle.
pub struct FrameBuffer {
    buf: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Try to return one complete decoded frame.
    ///
    /// Reads as many bytes as the stream offers (stopping on `WouldBlock` or
    /// `TimedOut`), then checks whether the accumulated buffer contains a full
    /// length-prefixed frame.
    ///
    /// Returns:
    /// * `Ok(Some(payload))` -- a complete, decompressed frame is ready.
    /// * `Ok(None)`          -- not enough bytes yet; call again after the next
    ///                          read opportunity.
    /// * `Err(_)`            -- a real I/O or protocol error occurred.
    pub fn try_read_frame(&mut self, stream: &mut impl Read) -> ZamResult<Option<Vec<u8>>> {
        // Fast path: if the buffer already holds a complete frame, return it
        // without touching the stream at all. This handles the case where a
        // previous read delivered two frames in one syscall.
        if let Some(frame) = self.try_consume_frame()? {
            return Ok(Some(frame));
        }

        // Drain whatever bytes the stream has right now into our buffer.
        let mut tmp = [0u8; 8192];
        let mut got_new_bytes = false;
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => {
                    // The peer sent us EOF (connection closed cleanly).
                    // Only treat it as a real EOF if we received no new bytes
                    // in this call. If we did receive some bytes and *then*
                    // got Ok(0), the OS drained the kernel buffer -- we process
                    // what we have and let the next poll see another Ok(0).
                    if !got_new_bytes {
                        return Err(ZamError::Io(std::io::Error::new(
                            ErrorKind::UnexpectedEof,
                            "connection closed by peer",
                        )));
                    }
                    break;
                }
                Ok(n) => {
                    self.buf.extend_from_slice(&tmp[..n]);
                    got_new_bytes = true;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    // Nothing more to read right now -- stop draining.
                    break;
                }
                Err(e) => return Err(ZamError::Io(e)),
            }
        }

        // Check the buffer again after draining.
        self.try_consume_frame()
    }

    /// Attempt to extract and decode one complete frame from `self.buf`.
    /// Returns `Ok(None)` if fewer than `4 + total_len` bytes are present.
    fn try_consume_frame(&mut self) -> ZamResult<Option<Vec<u8>>> {
        // Check whether we have accumulated a full frame.
        // Wire format: [4 bytes big-endian u32 = total_len] [1 byte flag] [body]
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let total_len =
            u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;

        if total_len == 0 {
            // Empty frame -- consume the 4-byte header and return an empty payload.
            self.buf.drain(..4);
            return Ok(Some(vec![]));
        }

        if total_len as u64 > MAX_FRAME_SIZE as u64 {
            return Err(ZamError::Protocol(format!(
                "received frame too large: {} bytes (max {})",
                total_len, MAX_FRAME_SIZE
            )));
        }

        let frame_end = 4 + total_len;
        if self.buf.len() < frame_end {
            // Not enough bytes yet -- wait for more.
            return Ok(None);
        }

        // We have a complete frame; decode it.
        let flag = self.buf[4];
        let body = self.buf[5..frame_end].to_vec();
        self.buf.drain(..frame_end);

        const FLAG_RAW: u8 = 0x00;
        const FLAG_ZSTD: u8 = 0x01;

        let payload = match flag {
            FLAG_RAW => body,
            FLAG_ZSTD => zstd::decode_all(body.as_slice())
                .map_err(|e| ZamError::Protocol(format!("zstd decompress: {e}")))?,
            other => {
                return Err(ZamError::Protocol(format!(
                    "unknown frame flag: 0x{other:02x}"
                )))
            }
        };

        Ok(Some(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::write_frame;
    use std::io::Cursor;

    fn make_frame(payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        write_frame(&mut buf, payload).unwrap();
        buf
    }

    #[test]
    fn test_complete_frame_at_once() {
        let payload = b"hello from bhutan";
        let wire = make_frame(payload);
        let mut fb = FrameBuffer::new();
        // Simulate one big read delivering everything.
        let result = fb.try_read_frame(&mut Cursor::new(&wire)).unwrap();
        assert_eq!(result, Some(payload.to_vec()));
        // Buffer should be empty now.
        assert!(fb.buf.is_empty());
    }

    #[test]
    fn test_two_frames_back_to_back() {
        let wire1 = make_frame(b"frame-one");
        let wire2 = make_frame(b"frame-two");
        let mut combined = wire1.clone();
        combined.extend_from_slice(&wire2);

        let mut fb = FrameBuffer::new();
        let r1 = fb.try_read_frame(&mut Cursor::new(&combined)).unwrap();
        assert_eq!(r1, Some(b"frame-one".to_vec()));
        // The second frame's bytes are still in fb.buf -- calling with an
        // empty reader should return the second frame from buffered data.
        let r2 = fb.try_read_frame(&mut Cursor::new(&[])).unwrap();
        assert_eq!(r2, Some(b"frame-two".to_vec()));
    }

    #[test]
    fn test_partial_header_returns_none() {
        let wire = make_frame(b"some data");
        let partial = &wire[..2]; // only 2 of the 4 header bytes
        let mut fb = FrameBuffer::new();
        let result = fb.try_read_frame(&mut Cursor::new(partial)).unwrap();
        assert!(result.is_none());
        assert_eq!(fb.buf.len(), 2);
    }

    #[test]
    fn test_partial_body_returns_none() {
        let wire = make_frame(b"some longer payload that has many bytes");
        let partial = &wire[..wire.len() - 5]; // missing last 5 bytes
        let mut fb = FrameBuffer::new();
        let result = fb.try_read_frame(&mut Cursor::new(partial)).unwrap();
        assert!(result.is_none());
        // Bytes are preserved in the buffer.
        assert_eq!(fb.buf.len(), partial.len());
    }

    #[test]
    fn test_split_delivery_reassembles_frame() {
        let payload = b"patient-record-from-rural-bhutan";
        let wire = make_frame(payload);
        let mid = wire.len() / 2;

        let mut fb = FrameBuffer::new();
        // First half -- should return None.
        let r1 = fb.try_read_frame(&mut Cursor::new(&wire[..mid])).unwrap();
        assert!(r1.is_none());
        // Second half -- should now return the full frame.
        let r2 = fb.try_read_frame(&mut Cursor::new(&wire[mid..])).unwrap();
        assert_eq!(r2, Some(payload.to_vec()));
    }

    #[test]
    fn test_empty_reader_on_empty_buffer_is_eof() {
        let mut fb = FrameBuffer::new();
        // Empty buffer + reader returning Ok(0) = peer closed the connection.
        // This is how the sync session's graceful-close loop detects the end.
        let result = fb.try_read_frame(&mut Cursor::new(&[]));
        assert!(matches!(result, Err(ZamError::Io(_))));
    }
}
