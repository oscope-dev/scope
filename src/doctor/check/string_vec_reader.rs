use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

/// This reader reads from a vector of strings, treating each string as a line of text.
/// This is useful because sometimes we no longer have direct access to the original IO
/// stream but still want to process the output as if it were read line by line.
pub struct StringVecReader<'a> {
    pub(crate) lines: &'a [String],
    pub(crate) line_idx: usize,
    pub(crate) byte_idx: usize,
    pub(crate) emitted_newline: bool,
}

impl<'a> StringVecReader<'a> {
    pub fn new(lines: &'a [String]) -> Self {
        Self {
            lines,
            line_idx: 0,
            byte_idx: 0,
            emitted_newline: false,
        }
    }
}

impl<'a> Read for StringVecReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < buf.len() && self.line_idx < self.lines.len() {
            let line = &self.lines[self.line_idx];
            let bytes = line.as_bytes();

            if self.byte_idx < bytes.len() {
                let to_copy = std::cmp::min(buf.len() - written, bytes.len() - self.byte_idx);
                buf[written..written + to_copy]
                    .copy_from_slice(&bytes[self.byte_idx..self.byte_idx + to_copy]);
                self.byte_idx += to_copy;
                written += to_copy;
            } else if !self.emitted_newline {
                buf[written] = b'\n';
                written += 1;
                self.emitted_newline = true;
            } else {
                self.line_idx += 1;
                self.byte_idx = 0;
                self.emitted_newline = false;
            }
        }
        Ok(written)
    }
}

impl<'a> AsyncRead for StringVecReader<'a> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Safety: StringVecReader does not implement Drop, so this is safe.
        // Pinning is only relevant for types that implement Drop or have self-referential
        // fields. Since StringVecReader does not implement Drop and does not contain
        // self-referential fields, it is safe to use `&mut *self` here.
        let this = &mut *self;
        let dst = buf.initialize_unfilled();
        match this.read(dst) {
            Ok(n) => {
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod read {
        use super::*;
        use std::io::Read;

        #[test]
        fn test_read_single_line() {
            let lines = vec!["hello".to_string()];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 16];
            let n = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"hello\n");
        }

        #[test]
        fn test_read_multiple_lines() {
            let lines = vec!["foo".to_string(), "bar".to_string()];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 16];
            let n = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"foo\nbar\n");
        }

        #[test]
        fn test_read_partial_buffer() {
            let lines = vec!["abcdef".to_string()];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 3];
            let n = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"abc");
            let n2 = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n2], b"def");
            let n3 = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n3], b"\n");
        }

        #[test]
        fn test_empty_lines() {
            let lines: Vec<String> = vec![];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 8];
            let n = reader.read(&mut buf).unwrap();
            assert_eq!(n, 0);
        }

        #[test]
        fn test_read_exact_buffer() {
            let lines = vec!["abc".to_string()];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 4];
            let n = reader.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"abc\n");
            let n2 = reader.read(&mut buf).unwrap();
            assert_eq!(n2, 0);
        }
    }

    mod async_read {
        use super::*;
        use std::pin::Pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        use tokio::io::{AsyncRead, ReadBuf};

        // Minimal waker for tests
        fn dummy_waker() -> Waker {
            fn no_op(_: *const ()) {}
            fn clone(_: *const ()) -> RawWaker {
                dummy_raw_waker()
            }
            fn dummy_raw_waker() -> RawWaker {
                RawWaker::new(std::ptr::null(), &VTABLE)
            }
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
            unsafe { Waker::from_raw(dummy_raw_waker()) }
        }

        fn poll_once<R: AsyncRead + Unpin>(
            mut reader: R,
            buf: &mut [u8],
        ) -> std::io::Result<usize> {
            let mut read_buf = ReadBuf::new(buf);
            let waker = dummy_waker();
            let mut cx = Context::from_waker(&waker);
            let pin = Pin::new(&mut reader);
            match AsyncRead::poll_read(pin, &mut cx, &mut read_buf) {
                Poll::Ready(Ok(())) => Ok(read_buf.filled().len()),
                Poll::Ready(Err(e)) => Err(e),
                Poll::Pending => panic!("poll_read returned Pending for in-memory reader"),
            }
        }

        #[test]
        fn test_poll_read_single_line() {
            let lines = vec!["hello".to_string()];
            let reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 16];
            let n = poll_once(reader, &mut buf).unwrap();
            assert_eq!(&buf[..n], b"hello\n");
        }

        #[test]
        fn test_poll_read_multiple_lines() {
            let lines = vec!["foo".to_string(), "bar".to_string()];
            let reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 16];
            let n = poll_once(reader, &mut buf).unwrap();
            assert_eq!(&buf[..n], b"foo\nbar\n");
        }

        #[test]
        fn test_poll_read_partial_buffer() {
            let lines = vec!["abcdef".to_string()];
            let mut reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 3];
            let n = poll_once(&mut reader, &mut buf).unwrap();
            assert_eq!(&buf[..n], b"abc");
            let n2 = poll_once(&mut reader, &mut buf).unwrap();
            assert_eq!(&buf[..n2], b"def");
            let n3 = poll_once(&mut reader, &mut buf).unwrap();
            assert_eq!(&buf[..n3], b"\n");
        }

        #[test]
        fn test_poll_read_empty_lines() {
            let lines: Vec<String> = vec![];
            let reader = StringVecReader::new(&lines);
            let mut buf = [0u8; 8];
            let n = poll_once(reader, &mut buf).unwrap();
            assert_eq!(n, 0);
        }
    }
}
