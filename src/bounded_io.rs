use std::io::{self, BufRead, Read};

pub(crate) const MAX_PROVIDER_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_PROVIDER_STDERR_BYTES: usize = 64 * 1024;

pub(crate) fn read_bounded_utf8_line<R: BufRead>(
    reader: &mut R,
    line: &mut String,
    max_bytes: usize,
) -> io::Result<usize> {
    let mut bytes = Vec::new();
    let read = reader
        .take(max_bytes.saturating_add(1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input frame exceeds the {max_bytes}-byte limit"),
        ));
    }
    let decoded = std::str::from_utf8(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input frame is not valid UTF-8: {error}"),
        )
    })?;
    line.clear();
    line.push_str(decoded);
    Ok(read)
}

pub(crate) fn read_bounded_utf8_tail<R: Read>(
    mut reader: R,
    max_bytes: usize,
) -> io::Result<String> {
    let mut tail = Vec::with_capacity(max_bytes.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        if max_bytes == 0 {
            continue;
        }
        if read >= max_bytes {
            tail.clear();
            tail.extend_from_slice(&chunk[read - max_bytes..read]);
            continue;
        }
        let overflow = tail.len().saturating_add(read).saturating_sub(max_bytes);
        if overflow > 0 {
            tail.drain(..overflow);
        }
        tail.extend_from_slice(&chunk[..read]);
    }
    Ok(String::from_utf8_lossy(&tail).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn bounded_line_accepts_the_limit_and_rejects_the_next_byte() {
        let mut line = String::new();
        let mut accepted = BufReader::new(Cursor::new(b"1234567\n"));
        assert_eq!(
            read_bounded_utf8_line(&mut accepted, &mut line, 8).unwrap(),
            8
        );
        assert_eq!(line, "1234567\n");

        let mut rejected = BufReader::new(Cursor::new(b"12345678\n"));
        let error = read_bounded_utf8_line(&mut rejected, &mut line, 8).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("8-byte limit"));
    }

    #[test]
    fn bounded_tail_drains_the_stream_without_retaining_its_prefix() {
        let tail = read_bounded_utf8_tail(Cursor::new(b"0123456789"), 4).unwrap();
        assert_eq!(tail, "6789");
    }
}
