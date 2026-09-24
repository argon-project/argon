use serde::{
    Deserialize, 
    Serialize,
    de,
    Deserializer,
    Serializer
};
use std::borrow::Cow;
use std::fmt;
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

impl Default for Encoding {
    fn default() -> Self {
        Self::UTF8
    }
}

impl<'de> Deserialize<'de> for Encoding {
    fn deserialize<D>(deserializer: D) -> Result<Encoding, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EncodingVisitor;

        impl<'de> de::Visitor<'de> for EncodingVisitor {
            type Value = Encoding;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("semver version")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Encoding::from_str(string).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(EncodingVisitor)
    }
}

impl Serialize for Encoding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

pub struct EncodingStrUnknown;

impl FromStr for Encoding {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "UTF8" | "utf8" | "UTF-8" | "utf-8" => Ok(Self::UTF8),
            "UTF16" | "utf16" | "UTF-16" | "utf-16" => Ok(Self::UTF16(Endianness::system())),
            "UTF16LE" | "utf16le" | "UTF-16le" | "utf-16le" => Ok(Self::UTF16(Endianness::LittleEndian)),
            "UTF16BE" | "utf16be" | "UTF-16be" | "utf-16be" => Ok(Self::UTF16(Endianness::BigEndian)),
            _ => Err(format!("Unknown encoding '{}'", s))
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

use core::{
    borrow::Borrow,
    hash::{Hash, Hasher},
    ops::Deref,
    str::from_utf8,
};

pub use pulldown_cmark::InlineStr;

/// A copy-on-write string that can be owned, borrowed
/// or inlined.
///
/// It is three words long.
#[derive(Debug, Eq)]
pub enum CowStr<'a> {
    /// An owned string
    Owned(String),
    /// Static string
    Static(&'static str),
    /// A borrowed string
    Borrowed(&'a str),
    /// A short inline string
    Inlined(pulldown_cmark::InlineStr),
}

#[cfg(feature = "serde")]
mod serde_impl {
    use core::fmt;

    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

    use super::CowStr;

    impl<'a> Serialize for CowStr<'a> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(self.as_ref())
        }
    }

    struct CowStrVisitor;

    impl<'de> de::Visitor<'de> for CowStrVisitor {
        type Value = CowStr<'de>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(CowStr::Borrowed(v))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match v.try_into() {
                Ok(it) => Ok(CowStr::Inlined(it)),
                Err(_) => Ok(CowStr::Owned(String::from(v).into_boxed_str())),
            }
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(CowStr::Owned(v.into_boxed_str()))
        }
    }

    impl<'a, 'de: 'a> Deserialize<'de> for CowStr<'a> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(CowStrVisitor)
        }
    }
}

impl<'a> AsRef<str> for CowStr<'a> {
    fn as_ref(&self) -> &str {
        self.deref()
    }
}

impl<'a> Hash for CowStr<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<'a> core::clone::Clone for CowStr<'a> {
    fn clone(&self) -> Self {
        match self {
            CowStr::Owned(s) => match InlineStr::try_from(&**s) {
                Ok(inline) => CowStr::Inlined(inline),
                Err(..) => CowStr::Owned(s.clone()),
            },
            CowStr::Static(s) => CowStr::Static(s),
            CowStr::Borrowed(s) => CowStr::Borrowed(s),
            CowStr::Inlined(s) => CowStr::Inlined(*s),
        }
    }
}

impl<'a> core::cmp::PartialEq<CowStr<'a>> for CowStr<'a> {
    fn eq(&self, other: &CowStr<'_>) -> bool {
        self.deref() == other.deref()
    }
}

impl<'a> From<&'a str> for CowStr<'a> {
    fn from(s: &'a str) -> Self {
        CowStr::Borrowed(s)
    }
}

impl<'a> From<String> for CowStr<'a> {
    fn from(s: String) -> Self {
        CowStr::Owned(s)
    }
}

impl<'a> From<char> for CowStr<'a> {
    fn from(c: char) -> Self {
        CowStr::Inlined(c.into())
    }
}

impl<'a> From<Cow<'a, str>> for CowStr<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        match s {
            Cow::Borrowed(s) => CowStr::Borrowed(s),
            Cow::Owned(s) => CowStr::Owned(s),
        }
    }
}

impl<'a> From<CowStr<'a>> for Cow<'a, str> {
    fn from(s: CowStr<'a>) -> Self {
        match s {
            CowStr::Owned(s) => Cow::Owned(s.to_string()),
            CowStr::Inlined(s) => Cow::Owned(s.to_string()),
            CowStr::Static(s) => Cow::Borrowed(s),
            CowStr::Borrowed(s) => Cow::Borrowed(s),
        }
    }
}

impl<'a> From<Cow<'a, char>> for CowStr<'a> {
    fn from(s: Cow<'a, char>) -> Self {
        CowStr::Inlined(InlineStr::from(*s))
    }
}

impl<'a> From<pulldown_cmark::CowStr<'a>> for CowStr<'a> {
    fn from(s: pulldown_cmark::CowStr<'a>) -> Self {
        match s {
            pulldown_cmark::CowStr::Boxed(s) => Self::Owned(s.into()),
            pulldown_cmark::CowStr::Inlined(s) => Self::Inlined(s),
            pulldown_cmark::CowStr::Borrowed(s) => Self::Borrowed(s),
        }
    }
}

impl<'a> From<CowStr<'a>> for pulldown_cmark::CowStr<'a> {
    fn from(s: CowStr<'a>) -> Self {
        match s {
            CowStr::Owned(s) => Self::Boxed(s.into()),
            CowStr::Borrowed(s) => Self::Borrowed(s),
            CowStr::Inlined(s) => Self::Inlined(s),
            CowStr::Static(s) => Self::Borrowed(s),
        }
    }
}

impl<'a> From<CowStr<'a>> for String {
    fn from(s: CowStr<'a>) -> Self {
        match s {
            CowStr::Owned(s) => s.into(),
            CowStr::Static(s) => s.into(),
            CowStr::Inlined(s) => s.as_ref().into(),
            CowStr::Borrowed(s) => s.into(),
        }
    }
}

impl<'a> From<&CowStr<'a>> for String {
    fn from(s: &CowStr<'a>) -> Self {
        s.string()
    }
}

impl<'a> Deref for CowStr<'a> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            CowStr::Owned(b) => b,
            CowStr::Static(b) => b,
            CowStr::Borrowed(b) => b,
            CowStr::Inlined(s) => s.deref(),
        }
    }
}

impl<'a> Borrow<str> for CowStr<'a> {
    fn borrow(&self) -> &str {
        self.deref()
    }
}

impl<'a> CowStr<'a> {
    pub fn into_string(self) -> String {
        match self {
            CowStr::Owned(b) => b.into(),
            CowStr::Borrowed(b) => b.to_owned(),
            CowStr::Static(b) => b.to_owned(),
            CowStr::Inlined(s) => s.deref().to_owned(),
        }
    }

    pub fn string(&self) -> String {
        match self {
            CowStr::Owned(b) => String::from(b.clone()),
            CowStr::Static(b) => String::from(*b),
            CowStr::Borrowed(b) => (*b).to_owned(),
            CowStr::Inlined(s) => s.deref().to_owned(),
        }
    }

    pub const fn len(&self) -> usize {
        match self {
            CowStr::Owned(b) => b.len(),
            CowStr::Static(b) => b.len(),
            CowStr::Borrowed(b) => b.len(),
            CowStr::Inlined(s) => s.len(),
        }
    }

    pub fn into_static(self) -> CowStr<'static> {
        match self {
            CowStr::Owned(b) => CowStr::Owned(b),
            CowStr::Static(b) => CowStr::Static(b),
            CowStr::Borrowed(b) => match InlineStr::try_from(b) {
                Ok(inline) => CowStr::Inlined(inline),
                Err(_) => CowStr::Owned(b.into()),
            },
            CowStr::Inlined(s) => CowStr::Inlined(s),
        }
    }
}

#[cfg(test)]
mod test_special_string {

    use super::*;

    #[test]
    fn inlinestr_ascii() {
        let s: InlineStr = 'a'.into();
        assert_eq!("a", s.deref());
    }

    #[test]
    fn inlinestr_unicode() {
        let s: InlineStr = '🍔'.into();
        assert_eq!("🍔", s.deref());
    }

    #[test]
    fn cowstr_size() {
        let size = core::mem::size_of::<CowStr>();
        let word_size = core::mem::size_of::<isize>();
        assert_eq!(3 * word_size, size);
    }

    #[test]
    fn cowstr_char_to_string() {
        let c = '藏';
        let smort: CowStr = c.into();
        let owned: String = smort.to_string();
        let expected = "藏".to_owned();
        assert_eq!(expected, owned);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn inlinestr_fits_twentytwo() {
        let s = "0123456789abcdefghijkl";
        let stack_str = InlineStr::try_from(s).unwrap();
        assert_eq!(stack_str.deref(), s);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn inlinestr_not_fits_twentythree() {
        let s = "0123456789abcdefghijklm";
        let _stack_str = InlineStr::try_from(s).unwrap_err();
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn small_boxed_str_clones_to_stack() {
        let s = "0123456789abcde".to_owned();
        let smort: CowStr = s.into();
        let smort_clone = smort.clone();

        if let CowStr::Inlined(..) = smort_clone {
        } else {
            panic!("Expected a Inlined variant!");
        }
    }

    #[test]
    fn cow_to_cow_str() {
        let s = "some text";
        let cow = Cow::Borrowed(s);
        let actual = CowStr::from(cow);
        let expected = CowStr::Borrowed(s);
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "some text".to_string();
        let cow: Cow<str> = Cow::Owned(s.clone());
        let actual = CowStr::from(cow);
        let expected = CowStr::Owned(s);
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    #[test]
    fn cow_str_to_cow() {
        let s = "some text";
        let cow_str = CowStr::Borrowed(s);
        let actual = Cow::from(cow_str);
        let expected = Cow::Borrowed(s);
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "s";
        let inline_str: InlineStr = InlineStr::try_from(s).unwrap();
        let cow_str = CowStr::Inlined(inline_str);
        let actual = Cow::from(cow_str);
        let expected: Cow<str> = Cow::Owned(s.to_string());
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let s = "s";
        let cow_str = CowStr::Owned(s.to_string());
        let actual = Cow::from(cow_str);
        let expected: Cow<str> = Cow::Owned(s.to_string());
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    #[test]
    fn cow_str_to_string() {
        let s = "some text";
        let cow_str = CowStr::Borrowed(s);
        let actual = String::from(cow_str);
        let expected = String::from("some text");
        assert_eq!(actual, expected);

        let s = "s";
        let inline_str: InlineStr = InlineStr::try_from(s).unwrap();
        let cow_str = CowStr::Inlined(inline_str);
        let actual = String::from(cow_str);
        let expected = String::from("s");
        assert_eq!(actual, expected);

        let s = "s";
        let cow_str = CowStr::Owned(s.to_string());
        let actual = String::from(cow_str);
        let expected = String::from("s");
        assert_eq!(actual, expected);
    }

    #[test]
    fn cow_char_to_cow_str() {
        let c = 'c';
        let cow: Cow<char> = Cow::Owned(c);
        let actual = CowStr::from(cow);
        let expected = CowStr::Inlined(InlineStr::from(c));
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));

        let c = 'c';
        let cow: Cow<char> = Cow::Borrowed(&c);
        let actual = CowStr::from(cow);
        let expected = CowStr::Inlined(InlineStr::from(c));
        assert_eq!(actual, expected);
        assert!(variant_eq(&actual, &expected));
    }

    fn variant_eq<T>(a: &T, b: &T) -> bool {
        core::mem::discriminant(a) == core::mem::discriminant(b)
    }
}
