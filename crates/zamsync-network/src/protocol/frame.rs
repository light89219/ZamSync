use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};
use zamsync_core::{ZamError, ZamResult};

pub const MAX_FRAME_SIZE: u32 = 64 * 1024 * 1024;

pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> ZamResult<()> {
    let len = payload.len() as u32;
    if len > MAX_FRAME_SIZE {
        return Err(ZamError::Protocol(format!(
            "frame too large: {} bytes (max {})",
            len, MAX_FRAME_SIZE
        )));
    }
    writer.write_u32::<BigEndian>(len)?;
    writer.write_all(payload)?;
    Ok(())
}

pub fn read_frame(reader: &mut impl Read) -> ZamResult<Vec<u8>> {
    let len = reader.read_u32::<BigEndian>()?;
    if len > MAX_FRAME_SIZE {
        return Err(ZamError::Protocol(format!(
            "received frame too large: {} bytes (max {})",
            len, MAX_FRAME_SIZE
        )));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_frame_roundtrip() {
        let payload = b"hello world";
        let mut buf = Vec::new();
        write_frame(&mut buf, payload).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = read_frame(&mut cursor).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_empty_frame() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &[]).unwrap();
        let mut cursor = Cursor::new(&buf);
        let decoded = read_frame(&mut cursor).unwrap();
        assert!(decoded.is_empty());
    }
}
