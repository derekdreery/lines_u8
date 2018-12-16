use std::io::{ErrorKind, BufRead, Result};

pub trait BufReadExt: BufRead + Sized {
    /// Like `read_until`, but fetches the next line, consumes \r, \n, or \r\n.
    ///
    /// The buf will not contain the newline, but the count will include 1 or 2 bytes for it, so if
    /// the count is 0 we are at EOF.
    fn read_line_u8(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        read_line_u8(self, buf)
    }

    fn lines_u8(self) -> LinesIter<Self> {
        LinesIter { inner: self }
    }
}

impl<R: BufRead> BufReadExt for R {}

pub struct LinesIter<R> {
    inner: R
}

impl<R> Iterator for LinesIter<R>
where R: BufRead
{
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = vec![];
        match self.inner.read_line_u8(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(Ok(line)),
            Err(e) => Some(Err(e))
        }
    }
}


fn read_line_u8<R: BufRead + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e)
            };
            let nl_idx = memchr::memchr(b'\n', available);
            let cr_idx = memchr::memchr(b'\r', available);
            match (nl_idx, cr_idx) {
                // \n
                (Some(nl_idx), Some(cr_idx)) if nl_idx < cr_idx => {
                    buf.extend_from_slice(&available[..nl_idx]);
                    (true, nl_idx + 1)
                }
                (Some(nl_idx), None) => {
                    buf.extend_from_slice(&available[..nl_idx]);
                    (true, nl_idx + 1)
                }
                // \r\n
                (Some(nl_idx), Some(cr_idx)) if cr_idx == nl_idx - 1 => {
                    buf.extend_from_slice(&available[..nl_idx-1]);
                    (true, nl_idx + 1)
                }
                // \r
                (Some(_), Some(cr_idx)) => {
                    buf.extend_from_slice(&available[..cr_idx]);
                    (true, cr_idx + 1)
                }
                // \r (?\n)
                (None, Some(cr_idx)) => {
                    // If we're at the end, we need to fetch more data, in case the next byte
                    // is '\n', so we just consume what we can and re-run the loop.
                    if available.len() == 1 {
                        // If there's only one byte, assume we've fetched all data. I'm pretty
                        // sure this is guaranteed. TODO check for sure.
                        buf.extend_from_slice(&available[..cr_idx]);
                        (true, 1)
                    } else if available.len() - 1 == cr_idx {
                        buf.extend_from_slice(&available[..cr_idx-1]);
                        (false, cr_idx - 1)
                    } else {
                        buf.extend_from_slice(&available[..cr_idx]);
                        (true, cr_idx + 1)
                    }
                }
                _ => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::BufReadExt;

    #[test]
    fn read_line_u8() {
        let mut text = Cursor::new("Some\r text\r\n\n\r");
        let mut line = vec![];
        assert_eq!(text.read_line_u8(&mut line).unwrap(), 5);
        assert_eq!(line, b"Some");
        line.clear();
        assert_eq!(text.read_line_u8(&mut line).unwrap(), 7);
        assert_eq!(line, b" text");
        line.clear();
        assert_eq!(text.read_line_u8(&mut line).unwrap(), 1);
        assert_eq!(line, b"");
        line.clear();
        assert_eq!(text.read_line_u8(&mut line).unwrap(), 1);
        assert_eq!(line, b"");
        line.clear();
        assert_eq!(text.read_line_u8(&mut line).unwrap(), 0);
    }

    #[test]
    fn lines_u8() {
        let text = Cursor::new("Some\r text\r\n\n\r");
        let mut iter = text.lines_u8();
        assert_eq!(iter.next().unwrap().unwrap(), b"Some");
        assert_eq!(iter.next().unwrap().unwrap(), b" text");
        assert_eq!(iter.next().unwrap().unwrap(), b"");
        assert_eq!(iter.next().unwrap().unwrap(), b"");
        assert!(iter.next().is_none());
    }
}
