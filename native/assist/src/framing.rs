use std::io::{self, Read, Write};

pub const MAX_FRAME: usize = 16_384;

// Little-endian length prefix. Reject before allocating; EOF mid-frame is fatal.
pub fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    read_bounded(reader, MAX_FRAME)
}

pub(crate) fn read_bounded(reader: &mut impl Read, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0u8; 4];
    match reader.read(&mut prefix[..1])? {
        0 => return Ok(None),
        1 => (),
        _ => unreachable!(),
    }
    reader.read_exact(&mut prefix[1..])?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame-size"));
    }
    let mut data = vec![0; length];
    reader.read_exact(&mut data)?;
    Ok(Some(data))
}

pub fn write_frame(writer: &mut impl Write, data: &[u8]) -> io::Result<()> {
    write_bounded(writer, data, MAX_FRAME)
}

pub(crate) fn write_bounded(writer: &mut impl Write, data: &[u8], limit: usize) -> io::Result<()> {
    if data.is_empty() || data.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame-size"));
    }
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(data)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_round_trip_and_eof() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"{} real bytes").unwrap();
        let mut reader = wire.as_slice();
        assert_eq!(read_frame(&mut reader).unwrap().unwrap(), b"{} real bytes");
        assert!(read_frame(&mut reader).unwrap().is_none());
    }
    #[test]
    fn oversized_empty_and_incomplete_are_refused() {
        for wire in [
            vec![0, 0, 0, 0],
            u32::MAX.to_le_bytes().to_vec(),
            vec![1],
            vec![8, 0, 0, 0, 42],
        ] {
            assert!(read_frame(&mut wire.as_slice()).is_err());
        }
    }
}
