use crate::output::OutputFormat;
use crate::sim::bit_table::BitTable;
use std::fmt;
use std::io::Read;
use std::marker::PhantomData;

const MAX_READ_REQUEST: usize = 64 * 1024;

#[derive(Debug)]
pub struct ResultFormatError {
    message: String,
}

impl ResultFormatError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ResultFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ResultFormatError {}

/// Streams strict `01`, `b8`, and `ptb64` result data into bounded shot blocks.
pub struct ResultBlockReader<R: Read> {
    input: R,
    width: usize,
    total_shots: u64,
    shots_read: u64,
    format: OutputFormat,
    max_chunk_shots: usize,
    ptb_group: Vec<u64>,
    ptb_group_shots: usize,
    ptb_group_offset: usize,
}

impl<R: Read> ResultBlockReader<R> {
    pub fn new(
        input: R,
        width: usize,
        total_shots: u64,
        format: OutputFormat,
        max_chunk_shots: usize,
    ) -> Result<Self, ResultFormatError> {
        if !matches!(
            format,
            OutputFormat::Format01 | OutputFormat::B8 | OutputFormat::Ptb64
        ) {
            return Err(ResultFormatError::new(format!(
                "unsupported result input format: {format:?}"
            )));
        }
        if max_chunk_shots == 0 {
            return Err(ResultFormatError::new("max_chunk_shots must be positive"));
        }
        if matches!(format, OutputFormat::Format01) {
            width
                .checked_add(1)
                .ok_or_else(|| ResultFormatError::new("01 row length overflows"))?;
        }
        if matches!(format, OutputFormat::B8) {
            width
                .checked_add(7)
                .ok_or_else(|| ResultFormatError::new("b8 row length overflows"))?;
        }

        Ok(Self {
            input,
            width,
            total_shots,
            shots_read: 0,
            format,
            max_chunk_shots,
            ptb_group: Vec::new(),
            ptb_group_shots: 0,
            ptb_group_offset: 0,
        })
    }

    pub fn next_block(&mut self) -> Result<Option<BitTable>, ResultFormatError> {
        if self.shots_read == self.total_shots {
            self.verify_eof()?;
            return Ok(None);
        }

        let remaining = self.total_shots - self.shots_read;
        let block_shots = remaining.min(self.max_chunk_shots as u64) as usize;
        let mut block = BitTable::try_new(self.width, block_shots).map_err(|err| {
            ResultFormatError::new(format!("BitTable allocation failed: {err:?}"))
        })?;

        match self.format {
            OutputFormat::Format01 => self.read_01_block(&mut block)?,
            OutputFormat::B8 => self.read_b8_block(&mut block)?,
            OutputFormat::Ptb64 => self.read_ptb64_block(&mut block)?,
            _ => unreachable!("format was validated by ResultBlockReader::new"),
        }

        self.shots_read += block_shots as u64;
        if self.shots_read == self.total_shots {
            self.verify_eof()?;
        }
        Ok(Some(block))
    }

    fn read_01_block(&mut self, block: &mut BitTable) -> Result<(), ResultFormatError> {
        let line_len = self.width + 1;
        let mut line = vec![0; line_len];
        for shot in 0..block.num_minor() {
            self.read_exact_bounded(&mut line)?;
            for bit in 0..self.width {
                match line[bit] {
                    b'0' => {}
                    b'1' => block.set(bit, shot, true),
                    byte => {
                        return Err(ResultFormatError::new(format!(
                            "invalid 01 byte 0x{byte:02x} at bit {bit}"
                        )));
                    }
                }
            }
            if line[self.width] != b'\n' {
                return Err(ResultFormatError::new(format!(
                    "01 row is missing newline after {} bits",
                    self.width
                )));
            }
        }
        Ok(())
    }

    fn read_b8_block(&mut self, block: &mut BitTable) -> Result<(), ResultFormatError> {
        let bytes_per_shot = (self.width + 7) / 8;
        let mut row = vec![0; bytes_per_shot];
        let used_bits_last_byte = self.width % 8;
        for shot in 0..block.num_minor() {
            self.read_exact_bounded(&mut row)?;
            if used_bits_last_byte != 0 && bytes_per_shot != 0 {
                let padding_mask = !((1u8 << used_bits_last_byte) - 1);
                if row[bytes_per_shot - 1] & padding_mask != 0 {
                    return Err(ResultFormatError::new("b8 row has nonzero padding bits"));
                }
            }
            for bit in 0..self.width {
                if (row[bit / 8] >> (bit % 8)) & 1 == 1 {
                    block.set(bit, shot, true);
                }
            }
        }
        Ok(())
    }

    fn read_ptb64_block(&mut self, block: &mut BitTable) -> Result<(), ResultFormatError> {
        let mut block_offset = 0;
        while block_offset < block.num_minor() {
            if self.ptb_group_offset == self.ptb_group_shots {
                let group_start = self.shots_read + block_offset as u64;
                let group_shots = (self.total_shots - group_start).min(64) as usize;
                self.read_ptb64_group(group_shots)?;
            }
            let count = (block.num_minor() - block_offset)
                .min(self.ptb_group_shots - self.ptb_group_offset);
            for bit in 0..self.width {
                let word = self.ptb_group[bit];
                for offset in 0..count {
                    if (word >> (self.ptb_group_offset + offset)) & 1 == 1 {
                        block.set(bit, block_offset + offset, true);
                    }
                }
            }
            block_offset += count;
            self.ptb_group_offset += count;
        }
        Ok(())
    }

    fn read_ptb64_group(&mut self, group_shots: usize) -> Result<(), ResultFormatError> {
        self.ptb_group.clear();
        self.ptb_group
            .try_reserve(self.width)
            .map_err(|_| ResultFormatError::new("ptb64 group allocation failed"))?;
        for bit in 0..self.width {
            let mut bytes = [0; 8];
            self.read_exact_bounded(&mut bytes)?;
            let word = u64::from_le_bytes(bytes);
            if group_shots < 64 && word >> group_shots != 0 {
                return Err(ResultFormatError::new(format!(
                    "ptb64 group has nonzero padding bits for bit {bit}"
                )));
            }
            self.ptb_group.push(word);
        }
        self.ptb_group_shots = group_shots;
        self.ptb_group_offset = 0;
        Ok(())
    }

    fn read_exact_bounded(&mut self, bytes: &mut [u8]) -> Result<(), ResultFormatError> {
        let mut start = 0;
        while start < bytes.len() {
            let end = (start + MAX_READ_REQUEST).min(bytes.len());
            match self.input.read(&mut bytes[start..end]) {
                Ok(0) => return Err(ResultFormatError::new("unexpected end of result input")),
                Ok(read) => start += read,
                Err(err) => {
                    return Err(ResultFormatError::new(format!(
                        "failed reading result input: {err}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn verify_eof(&mut self) -> Result<(), ResultFormatError> {
        let mut extra = [0; 1];
        match self.input.read(&mut extra) {
            Ok(0) => Ok(()),
            Ok(_) => Err(ResultFormatError::new(
                "result input has data after declared shots",
            )),
            Err(err) => Err(ResultFormatError::new(format!(
                "failed reading result input: {err}"
            ))),
        }
    }
}

/// Output selection reserved for the result writer implemented in Task 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultOutputKind {
    Measurements,
    Detectors,
    Observables,
}

/// Temporary API placeholder so reader-only tests compile with the integration contract.
pub struct ResultBlockWriter<W> {
    _output: PhantomData<W>,
}

impl<W> ResultBlockWriter<W> {
    pub fn new(
        _output: W,
        _kind: ResultOutputKind,
        _format: OutputFormat,
    ) -> Result<Self, ResultFormatError> {
        Ok(Self {
            _output: PhantomData,
        })
    }

    pub fn write_block<T>(&mut self, _block: &T) -> Result<(), ResultFormatError> {
        Err(ResultFormatError::new(
            "ResultBlockWriter is not implemented yet",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{OutputFormat, write_shots_01, write_shots_b8, write_shots_ptb64};
    use crate::sim::bit_table::BitTable;
    use std::io::{self, Cursor, Read};

    #[test]
    fn reads_01_in_bounded_blocks() {
        let expected = sample_table(3, 5);
        let mut input = Vec::new();
        write_shots_01(&expected, &mut input).unwrap();

        let actual = read_all(input, 3, 5, OutputFormat::Format01, 2).unwrap();

        assert_tables_equal(&actual, &expected);
    }

    #[test]
    fn rejects_invalid_01_bytes_and_newline_positions() {
        for input in [
            b"10x\n".as_slice(),
            b"1\n0\n".as_slice(),
            b"10\r".as_slice(),
        ] {
            assert!(
                read_all(input, 2, 1, OutputFormat::Format01, 1).is_err(),
                "input={input:?}"
            );
        }
    }

    #[test]
    fn rejects_short_and_extra_01_input() {
        assert!(read_all(b"10\n0", 2, 2, OutputFormat::Format01, 2).is_err());
        assert!(read_all(b"10\n01\n00\n", 2, 2, OutputFormat::Format01, 2).is_err());
    }

    #[test]
    fn reads_b8_partial_rows_and_rejects_padding() {
        let expected = sample_table(10, 3);
        let mut input = Vec::new();
        write_shots_b8(&expected, &mut input).unwrap();
        assert_tables_equal(
            &read_all(&input, 10, 3, OutputFormat::B8, 2).unwrap(),
            &expected,
        );

        let mut padded = input;
        padded[1] |= 0xfc;
        assert!(read_all(padded, 10, 3, OutputFormat::B8, 2).is_err());
    }

    #[test]
    fn rejects_short_and_extra_b8_input() {
        assert!(read_all([0b0000_0001], 9, 1, OutputFormat::B8, 1).is_err());
        assert!(read_all([0, 0], 1, 1, OutputFormat::B8, 1).is_err());
    }

    #[test]
    fn reads_ptb64_across_groups_and_small_blocks() {
        let expected = sample_table(3, 66);
        let mut input = Vec::new();
        write_shots_ptb64(&expected, &mut input).unwrap();

        let actual = read_all(input, 3, 66, OutputFormat::Ptb64, 17).unwrap();

        assert_tables_equal(&actual, &expected);
    }

    #[test]
    fn rejects_ptb64_bad_length_padding_and_extra_groups() {
        let expected = sample_table(3, 65);
        let mut input = Vec::new();
        write_shots_ptb64(&expected, &mut input).unwrap();

        assert!(read_all(&input[..input.len() - 1], 3, 65, OutputFormat::Ptb64, 64).is_err());
        assert!(
            read_all(
                [input.as_slice(), input.as_slice()].concat(),
                3,
                65,
                OutputFormat::Ptb64,
                64
            )
            .is_err()
        );

        let mut invalid_padding = input;
        let final_group_offset = 3 * 8;
        invalid_padding[final_group_offset] |= 0b0000_0010;
        assert!(read_all(invalid_padding, 3, 65, OutputFormat::Ptb64, 64).is_err());
    }

    #[test]
    fn zero_width_is_empty_for_binary_formats_and_newline_framed_for_01() {
        for format in [OutputFormat::B8, OutputFormat::Ptb64] {
            let actual = read_all([], 0, 3, format, 2).unwrap();
            assert_eq!(actual.num_major(), 0);
            assert_eq!(actual.num_minor(), 3);
            assert!(read_all([0], 0, 3, format, 2).is_err());
        }

        let actual = read_all(b"\n\n\n", 0, 3, OutputFormat::Format01, 2).unwrap();
        assert_eq!(actual.num_major(), 0);
        assert_eq!(actual.num_minor(), 3);
        assert!(read_all(b"\n\n", 0, 3, OutputFormat::Format01, 2).is_err());
    }

    #[test]
    fn rejects_unsupported_formats_and_zero_chunk_size() {
        assert!(ResultBlockReader::new(Cursor::new([]), 1, 0, OutputFormat::R8, 1).is_err());
        assert!(ResultBlockReader::new(Cursor::new([]), 1, 0, OutputFormat::B8, 0).is_err());
    }

    #[test]
    fn never_requests_more_than_64_kib_and_handles_short_reads() {
        let expected = sample_table(9, 65_537);
        let mut input = Vec::new();
        write_shots_b8(&expected, &mut input).unwrap();
        let guarded = GuardedRead::new(input, 64 * 1024, 7);

        let actual = read_all_reader(guarded, 9, 65_537, OutputFormat::B8, 127).unwrap();

        assert_tables_equal(&actual, &expected);
    }

    fn read_all(
        input: impl AsRef<[u8]>,
        width: usize,
        shots: u64,
        format: OutputFormat,
        max_chunk_shots: usize,
    ) -> Result<BitTable, ResultFormatError> {
        read_all_reader(
            Cursor::new(input.as_ref()),
            width,
            shots,
            format,
            max_chunk_shots,
        )
    }

    fn read_all_reader<R: Read>(
        input: R,
        width: usize,
        shots: u64,
        format: OutputFormat,
        max_chunk_shots: usize,
    ) -> Result<BitTable, ResultFormatError> {
        let mut reader = ResultBlockReader::new(input, width, shots, format, max_chunk_shots)?;
        let mut result = BitTable::try_new(width, shots as usize)
            .map_err(|err| ResultFormatError::new(format!("result allocation failed: {err:?}")))?;
        let mut first_shot = 0;
        while let Some(block) = reader.next_block()? {
            for bit in 0..width {
                for shot in 0..block.num_minor() {
                    result.set(bit, first_shot + shot, block.get(bit, shot));
                }
            }
            first_shot += block.num_minor();
        }
        assert_eq!(first_shot, shots as usize);
        Ok(result)
    }

    fn sample_table(width: usize, shots: usize) -> BitTable {
        let mut table = BitTable::new(width, shots);
        for bit in 0..width {
            for shot in 0..shots {
                table.set(bit, shot, (bit * 17 + shot * 11) % 5 < 2);
            }
        }
        table
    }

    fn assert_tables_equal(actual: &BitTable, expected: &BitTable) {
        assert_eq!(actual.num_major(), expected.num_major());
        assert_eq!(actual.num_minor(), expected.num_minor());
        for bit in 0..actual.num_major() {
            for shot in 0..actual.num_minor() {
                assert_eq!(
                    actual.get(bit, shot),
                    expected.get(bit, shot),
                    "bit={bit} shot={shot}"
                );
            }
        }
    }

    struct GuardedRead {
        input: Cursor<Vec<u8>>,
        max_request: usize,
        max_yield: usize,
    }

    impl GuardedRead {
        fn new(input: Vec<u8>, max_request: usize, max_yield: usize) -> Self {
            Self {
                input: Cursor::new(input),
                max_request,
                max_yield,
            }
        }
    }

    impl Read for GuardedRead {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.len() > self.max_request {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "read request too large",
                ));
            }
            let max_yield = buf.len().min(self.max_yield);
            self.input.read(&mut buf[..max_yield])
        }
    }
}
