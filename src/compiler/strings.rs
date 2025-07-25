use tree_sitter;
use std::ops::Range;
use std::str::{
    FromStr, Utf8Error
};
use std::string::{
    FromUtf16Error
};
use std::error;


pub use tree_sitter::Point;
pub type Location = tree_sitter::Range;

pub trait ByteRangeProviding {
    fn byte_range(&self) -> Range<usize>;
}

impl ByteRangeProviding for Location {
    #[inline]
    fn byte_range(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Encoding {
    UTF8,
    UTF16(Endianness),
}

pub struct EncodingStrUnknown;

impl FromStr for Encoding {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "UTF8" | "utf8" | "UTF-8" | "utf-8" => Ok(Self::UTF8),
            "UTF16" | "utf16" | "UTF-16" | "utf-16" => Ok(Self::UTF16(Endianness::system())),
            "UTF16LE" | "utf16le" | "UTF-16le" | "utf-16le" => Ok(Self::UTF16(Endianness::LittleEndian)),
            "UTF16BE" | "utf16be" | "UTF-16be" | "utf-16be" => Ok(Self::UTF16(Endianness::BigEndian)),
            _ => Err("Unknown encoding")
        }
    }
}

impl Encoding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UTF8 => "UTF-8",
            Self::UTF16(Endianness::BigEndian) => "UTF-16BE",
            Self::UTF16(Endianness::LittleEndian) => "UTF-16LE",
        }
    }
}

impl ToString for Encoding {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Endianness {
    BigEndian,
    LittleEndian
}

impl Endianness {
    pub fn system() -> Self {
        if cfg!(target_endian = "big") {
            Self::BigEndian
        } else {
            Self::LittleEndian
        }
    }
}

pub trait Source: Copy {
    type StrError: error::Error;
    type AsSlice: AsRef<[u8]>;
    type AsStrSlice: AsRef<str> + ToString;
    type CodeUnit;

    fn string(&self, location: impl ByteRangeProviding) -> Result<String, Self::StrError>;
    fn slice(&self, location: impl ByteRangeProviding) -> Result<Self::AsSlice, Self::StrError>;
    fn str(&self, location: impl ByteRangeProviding) -> Result<Self::AsStrSlice, Self::StrError>;

    fn encoded_slice(&self, offset: usize, point: Point) -> impl AsRef<[Self::CodeUnit]>;
}

impl<'a> Source for &'a [u8] {
    type StrError = Utf8Error;
    type AsSlice = Self;
    type AsStrSlice = &'a str;
    type CodeUnit = u8;

    fn string(&self, location: impl ByteRangeProviding) -> Result<String, Utf8Error> {
        let s = str::from_utf8(&self[location.byte_range()])?;
        Ok(s.into())
    }

    fn slice(&self, location: impl ByteRangeProviding) -> Result<Self, Utf8Error> {
        Ok(&self[location.byte_range()])
    }

    fn str(&self, location: impl ByteRangeProviding) -> Result<Self::AsStrSlice, Self::StrError> {
        str::from_utf8(&self[location.byte_range()])
    }

    fn encoded_slice(&self, offset: usize, _point: Point) -> impl AsRef<[Self::CodeUnit]> {
        (offset < self.len()).then(|| &self[offset..]).unwrap_or_default()
    }
}

impl Source for &[u16] {
    type StrError = FromUtf16Error;
    type AsSlice = String;
    type AsStrSlice = String;
    type CodeUnit = u16;

    fn string(&self, location: impl ByteRangeProviding) -> Result<String, FromUtf16Error> {
        String::from_utf16(&self[location.byte_range().start / 2..location.byte_range().end / 2])
    }

    fn slice(&self, location: impl ByteRangeProviding) -> Result<String, FromUtf16Error> {
        self.string(location)
    }

    fn str(&self, location: impl ByteRangeProviding) -> Result<Self::AsStrSlice, Self::StrError> {
        self.string(location)
    }

    fn encoded_slice(&self, offset: usize, _point: Point) -> impl AsRef<[Self::CodeUnit]> {
        (offset < self.len()).then(|| &self[offset..]).unwrap_or_default()
    }
}