pub struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    fn read<T: Copy>(&mut self) -> T {
        let size = std::mem::size_of::<T>();
        let end = self.pos + size;
        let bytes = &self.data[self.pos..end];
        self.pos = end;

        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const T) }
    }

    pub fn read_u32(&mut self) -> u32 {
        u32::from_le(self.read::<u32>())
    }

    pub fn read_i32(&mut self) -> i32 {
        i32::from_le(self.read::<i32>())
    }

    pub fn read_i16(&mut self) -> i16 {
        i16::from_le(self.read::<i16>())
    }

    pub fn read_u16(&mut self) -> u16 {
        u16::from_le(self.read::<u16>())
    }

    pub fn read_u8(&mut self) -> u8 {
        u8::from_le(self.read::<u8>())
    }

    pub fn read_f32(&mut self) -> f32 {
        let bytes = self.read_bytes(4);
        let arr: [u8; 4] = bytes.try_into().unwrap();
        f32::from_le_bytes(arr)
    }

    pub fn read_string(&mut self, len: usize) -> String {
        let bytes = self.read_bytes(len);
        String::from_utf8_lossy(bytes).into_owned()
    }

    pub fn read_fixed_string(&mut self, len: usize) -> String {
        let remaining_bytes = self.data.len().saturating_sub(self.pos);
        let actual_len = len.min(remaining_bytes);

        if actual_len == 0 {
            eprintln!("Found actual len 0 for a fixed string.");
            return String::new();
        }

        let bytes = self.read_bytes(actual_len);
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(actual_len);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    pub fn read_null_terminated_string(&mut self) -> String {
        let start = self.pos;
        let mut end = start;

        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }

        let bytes = &self.data[start..end];

        self.pos = if end < self.data.len() { end + 1 } else { end };

        String::from_utf8_lossy(bytes).into_owned()
    }

    pub fn read_bytes(&mut self, len: usize) -> &[u8] {
        let start = self.pos;
        self.pos += len;
        &self.data[start..start + len]
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn seek_relative(&mut self, pos: usize) {
        self.pos += pos;
    }
}
