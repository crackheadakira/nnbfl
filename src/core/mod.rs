mod cursor;
mod error;
mod section;
mod tests;
mod writer;

pub use cursor::Cursor;
pub use error::{FormatError, NnbflError};
pub use section::SectionHeader;
pub use writer::Writer;

pub const fn tchar_code32(b: &[u8; 4]) -> u32 {
    (b[0] as u32) | ((b[1] as u32) << 8) | ((b[2] as u32) << 16) | ((b[3] as u32) << 24)
}

pub trait ReadWriteable: Sized + serde::Serialize + serde::de::DeserializeOwned {
    const EXTENSION: &'static str;

    fn parse(file: &[u8]) -> Result<Self, FormatError>;
    fn write(&self) -> writer::Writer;
}
