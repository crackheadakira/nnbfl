use crate::core::{FormatError, context::ContextStore};

#[derive(Default)]
pub struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub version: u32,
    pub section_start: Option<usize>,
    pub string_pool_start: Option<usize>,
    pub optional_offset: usize,
    pub last_was_pane: bool,
    pub is_embed: bool,
    pub contexts: ContextStore,
}

impl<'a> Cursor<'a> {
    pub fn ctx_section_start<T>(&self) -> Result<usize, FormatError> {
        self.section_start.ok_or_else(|| {
            let full_name = std::any::type_name::<T>();
            let short_name = full_name.split("::").last().unwrap_or(full_name);

            FormatError::MissingContext {
                expected: short_name,
                context: "section_start anchor",
                offset: self.pos,
            }
        })
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], FormatError> {
        let end = self.pos + len;

        if end > self.data.len() {
            return Err(FormatError::UnexpectedEof {
                offset: self.pos,
                requested_bytes: len,
            });
        }

        let slice = &self.data[self.pos..end];
        self.pos = end;

        Ok(slice)
    }

    pub fn read_u8(&mut self) -> Result<u8, FormatError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, FormatError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u32(&mut self) -> Result<u32, FormatError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_u64(&mut self) -> Result<u64, FormatError> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_i16(&mut self) -> Result<i16, FormatError> {
        let b = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_i32(&mut self) -> Result<i32, FormatError> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_f32(&mut self) -> Result<f32, FormatError> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_string(&mut self, len: usize) -> Result<String, FormatError> {
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn read_fixed_string(&mut self, len: usize) -> Result<String, FormatError> {
        let remaining_bytes = self.data.len().saturating_sub(self.pos);

        if remaining_bytes == 0 && len > 0 {
            return Err(FormatError::UnexpectedEof {
                offset: self.pos,
                requested_bytes: len,
            });
        }

        let actual_len = len.min(remaining_bytes);
        let bytes = self.read_bytes(actual_len)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(actual_len);

        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }

    pub fn read_null_terminated_string(&mut self) -> Result<String, FormatError> {
        let start = self.pos;
        let mut end = start;

        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }
        if end >= self.data.len() {
            return Err(FormatError::MalformedSection {
                section_type: "StringPool".to_owned(),
                offset: start,
                reason: "Unterminated string literal reached EOF".to_owned(),
            });
        }

        let bytes = &self.data[start..end];
        self.pos = end + 1;

        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn seek(&mut self, pos: usize) -> Result<(), FormatError> {
        if pos > self.data.len() {
            return Err(FormatError::UnexpectedEof {
                offset: self.data.len(),
                requested_bytes: pos - self.data.len(),
            });
        }

        self.pos = pos;
        Ok(())
    }

    pub fn seek_relative(&mut self, bytes: usize) {
        self.pos += bytes;
    }

    pub fn at<R>(
        &mut self,
        offset: usize,
        f: impl FnOnce(&mut Self) -> Result<R, FormatError>,
    ) -> Result<R, FormatError> {
        let saved = self.pos;
        self.seek(offset)?;

        let result = f(self);

        self.pos = saved;
        result
    }

    pub fn read_string_from_pool(&mut self) -> Result<String, FormatError> {
        let offset = self.read_u32()?;
        self.read_string_from_pool_by_offset(offset)
    }

    pub fn read_string_from_pool_by_offset(&mut self, offset: u32) -> Result<String, FormatError> {
        let start = self.string_pool_start.ok_or(FormatError::MissingContext {
            expected: "String",
            context: "string_pool_start",
            offset: self.pos,
        })?;

        let address = start + offset as usize;

        if address >= self.data.len() {
            return Err(FormatError::MalformedSection {
                section_type: "StringPool".into(),
                offset: self.pos,
                reason: format!("Offset {offset} causes string address overflow"),
            });
        }

        self.at(address, |c| c.read_null_terminated_string())
    }

    pub fn context<T: std::any::Any>(&self) -> Result<&T, FormatError> {
        self.contexts.get::<T>().ok_or(FormatError::MissingContext {
            expected: std::any::type_name::<T>(),
            context: "typed context",
            offset: self.pos,
        })
    }

    pub fn with_context<T: std::any::Any + Send + Sync, R>(
        &mut self,
        value: T,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.contexts.replace(value);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));

        self.contexts.restore::<T>(previous);

        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
