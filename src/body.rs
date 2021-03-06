//! Provides types for working with request and response bodies.

use bytes::Bytes;
use crate::error::Error;
use crate::internal;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::str;

/// Contains the body of an HTTP request or response.
///
/// This type is used to encapsulate the underlying stream or region of memory where the contents of the body is stored.
/// A `Body` can be created from many types of sources using the [`Into`](std::convert::Into) trait.
pub struct Body(Inner);

enum Inner {
    /// An empty body.
    Empty,
    /// A body stored in memory.
    Bytes(Cursor<Bytes>),
    /// A body read from a stream.
    Streaming(Box<Read + Send>),
}

impl Body {
    /// Create a body from a reader.
    pub fn from_reader(reader: impl Read + Send + 'static) -> Body {
        Body(Inner::Streaming(Box::new(reader)))
    }

    /// Report if this body is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }

    /// Get the size of the body, if known.
    pub fn len(&self) -> Option<usize> {
        match &self.0 {
            Inner::Empty => Some(0),
            Inner::Bytes(bytes) => Some(bytes.get_ref().len()),
            Inner::Streaming(_) => None,
        }
    }

    /// If this body is repeatable, reset the body stream back to the start of
    /// the content. Returns `false` if the body cannot be reset.
    pub fn reset(&mut self) -> bool {
        match &mut self.0 {
            Inner::Empty => true,
            Inner::Bytes(bytes) => {
                bytes.set_position(0);
                true
            },
            _ => false,
        }
    }

    /// Get the response body as a string.
    ///
    /// If the body comes from a stream, the steam bytes will be consumed and this method will return an empty string
    /// next call. If this body supports seeking, you can seek to the beginning of the body if you need to call this
    /// method again later.
    pub fn text(&mut self) -> Result<String, Error> {
        match &mut self.0 {
            Inner::Empty => Ok(String::new()),
            Inner::Bytes(bytes) => str::from_utf8(bytes.get_ref())
                .map(Into::into)
                .map_err(Into::into),
            Inner::Streaming(reader) => {
                let mut string = String::new();
                reader.read_to_string(&mut string)?;
                Ok(string)
            },
        }
    }

    /// Attempt to parse the response as JSON.
    #[cfg(feature = "json")]
    pub fn json(&mut self) -> Result<json::JsonValue, Error> {
        let text = self.text()?;
        Ok(json::parse(&text)?)
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Inner::Empty => Ok(0),
            Inner::Bytes(bytes) => bytes.read(buf),
            Inner::Streaming(reader) => reader.read(buf),
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Body(Inner::Empty)
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl From<Vec<u8>> for Body {
    fn from(body: Vec<u8>) -> Self {
        Bytes::from(body).into()
    }
}

impl<'a> From<&'a [u8]> for Body {
    fn from(body: &'a [u8]) -> Self {
        Bytes::from(body).into()
    }
}

impl From<Bytes> for Body {
    fn from(body: Bytes) -> Self {
        Body(Inner::Bytes(Cursor::new(body)))
    }
}

impl From<String> for Body {
    fn from(body: String) -> Self {
        body.into_bytes().into()
    }
}

impl<'a> From<&'a str> for Body {
    fn from(body: &'a str) -> Self {
        body.as_bytes().into()
    }
}

impl From<File> for Body {
    fn from(body: File) -> Self {
        Self::from_reader(body)
    }
}

impl<T: Into<Body>> From<Option<T>> for Body {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::default(),
        }
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            Inner::Empty => write!(f, "Empty"),
            Inner::Bytes(bytes) => write!(f, "Memory({})", internal::format_byte_string(bytes.get_ref())),
            Inner::Streaming(_) => write!(f, "Streaming"),
        }
    }
}
