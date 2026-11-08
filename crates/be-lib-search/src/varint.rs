use crate::Error;

pub(crate) fn put(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }

    pub(crate) fn varint(&mut self) -> Result<u64, Error> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self.bytes.get(self.at).ok_or(Error::Corrupt("varint"))?;
            self.at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte < 0x80 {
                return Ok(value);
            }
        }
        Err(Error::Corrupt("varint"))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        u32::try_from(self.varint()?).map_err(|_| Error::Corrupt("varint"))
    }

    pub(crate) fn byte(&mut self) -> Result<u8, Error> {
        let byte = *self.bytes.get(self.at).ok_or(Error::Corrupt("byte"))?;
        self.at += 1;
        Ok(byte)
    }

    pub(crate) fn bytes(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let slice = self
            .bytes
            .get(self.at..self.at + len)
            .ok_or(Error::Corrupt("bytes"))?;
        self.at += len;
        Ok(slice)
    }
}
