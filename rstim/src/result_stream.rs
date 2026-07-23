use crate::measurement_transform::DecodedSampleBlock;
use crate::output::OutputFormat;
use crate::sim::bit_table::BitTable;
use std::fmt;
use std::io::{Read, Write};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultOutputKind {
    Measurements,
    Detectors,
    Observables,
}

pub struct ResultBlockWriter<W: Write> {
    output: W,
    kind: ResultOutputKind,
    format: OutputFormat,
    ptb64_width: Option<usize>,
    ptb64_pending: Option<Ptb64Pending>,
}

#[derive(Clone)]
struct Ptb64Pending {
    words: Vec<u64>,
    shots: usize,
}

impl<W: Write> ResultBlockWriter<W> {
    pub fn new(
        output: W,
        kind: ResultOutputKind,
        format: OutputFormat,
    ) -> Result<Self, ResultFormatError> {
        if matches!(format, OutputFormat::Dets) && !matches!(kind, ResultOutputKind::Detectors) {
            return Err(ResultFormatError::new(
                "dets output format is only supported for detector results",
            ));
        }
        Ok(Self {
            output,
            kind,
            format,
            ptb64_width: None,
            ptb64_pending: None,
        })
    }

    pub fn write_block(&mut self, block: &DecodedSampleBlock) -> Result<(), ResultFormatError> {
        validate_block_shots(block)?;

        if matches!(self.format, OutputFormat::Ptb64) {
            return self.write_ptb64_block(self.selected_table(block));
        }

        let mut staging = Vec::new();
        match (self.kind, self.format) {
            (ResultOutputKind::Detectors, OutputFormat::Dets) => crate::output::write_shots_dets(
                &block.detections,
                &block.observable_flips,
                &mut staging,
            ),
            (_, OutputFormat::Format01) => {
                crate::output::write_shots_01(self.selected_table(block), &mut staging)
            }
            (_, OutputFormat::B8) => {
                crate::output::write_shots_b8(self.selected_table(block), &mut staging)
            }
            (_, OutputFormat::R8) => {
                crate::output::write_shots_r8(self.selected_table(block), &mut staging)
            }
            (_, OutputFormat::Hits) => {
                crate::output::write_shots_hits(self.selected_table(block), &mut staging)
            }
            (_, OutputFormat::Dets | OutputFormat::Ptb64) => unreachable!("format was validated"),
        }
        .map_err(write_error)?;
        self.output.write_all(&staging).map_err(write_error)
    }

    pub fn finish(&mut self) -> Result<(), ResultFormatError> {
        if matches!(self.format, OutputFormat::Ptb64) {
            self.flush_ptb64_pending()?;
        }
        self.output.flush().map_err(write_error)
    }

    fn selected_table<'a>(&self, block: &'a DecodedSampleBlock) -> &'a BitTable {
        match self.kind {
            ResultOutputKind::Measurements => &block.measurements,
            ResultOutputKind::Detectors => &block.detections,
            ResultOutputKind::Observables => &block.observable_flips,
        }
    }

    fn write_ptb64_block(&mut self, table: &BitTable) -> Result<(), ResultFormatError> {
        let width = table.num_major();
        if let Some(previous_width) = self.ptb64_width {
            if previous_width != width {
                return Err(ResultFormatError::new(
                    "ptb64 result width changes between blocks",
                ));
            }
        }

        let mut pending = self.ptb64_pending.clone().unwrap_or(Ptb64Pending {
            words: vec![0; width],
            shots: 0,
        });
        let mut staging = Vec::new();
        for shot in 0..table.num_minor() {
            for bit in 0..width {
                if table.get(bit, shot) {
                    pending.words[bit] |= 1u64 << pending.shots;
                }
            }
            pending.shots += 1;
            if pending.shots == 64 {
                append_ptb64_group(&pending.words, &mut staging);
                pending.words.fill(0);
                pending.shots = 0;
            }
        }

        self.output.write_all(&staging).map_err(write_error)?;
        self.ptb64_width = Some(width);
        self.ptb64_pending = Some(pending);
        Ok(())
    }

    fn flush_ptb64_pending(&mut self) -> Result<(), ResultFormatError> {
        let Some(pending) = &self.ptb64_pending else {
            return Ok(());
        };
        if pending.shots == 0 {
            return Ok(());
        }

        let mut staging = Vec::new();
        append_ptb64_group(&pending.words, &mut staging);
        self.output.write_all(&staging).map_err(write_error)?;
        self.ptb64_pending = Some(Ptb64Pending {
            words: vec![0; pending.words.len()],
            shots: 0,
        });
        Ok(())
    }
}

fn validate_block_shots(block: &DecodedSampleBlock) -> Result<(), ResultFormatError> {
    let measurements = block.measurements.num_minor();
    let detections = block.detections.num_minor();
    let observables = block.observable_flips.num_minor();
    if measurements != detections || measurements != observables {
        return Err(ResultFormatError::new(format!(
            "result block shot counts differ: measurements={measurements}, detections={detections}, observables={observables}",
        )));
    }
    Ok(())
}

fn append_ptb64_group(words: &[u64], output: &mut Vec<u8>) {
    for word in words {
        output.extend_from_slice(&word.to_le_bytes());
    }
}

fn write_error(error: std::io::Error) -> ResultFormatError {
    ResultFormatError::new(format!("failed writing result output: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measurement_transform::DecodedSampleBlock;
    use crate::output::{write_shots_01, write_shots_b8, write_shots_ptb64, OutputFormat};
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

    #[test]
    fn writes_detector_dets_with_detector_and_observable_labels() {
        let block = decoded_block(
            BitTable::new(0, 2),
            table_from_shots(&[&[true, false], &[false, true]]),
            table_from_shots(&[&[false, true], &[true, false]]),
        );
        let mut output = Vec::new();
        let mut writer =
            ResultBlockWriter::new(&mut output, ResultOutputKind::Detectors, OutputFormat::Dets)
                .unwrap();

        writer.write_block(&block).unwrap();
        writer.finish().unwrap();

        assert_eq!(output, b"shot D0 L1\nshot D1 L0\n");
    }

    #[test]
    fn writes_zero_detector_dets_with_observables() {
        let block = decoded_block(
            BitTable::new(0, 2),
            BitTable::new(0, 2),
            table_from_shots(&[&[true, false], &[false, true]]),
        );
        let mut output = Vec::new();
        let mut writer =
            ResultBlockWriter::new(&mut output, ResultOutputKind::Detectors, OutputFormat::Dets)
                .unwrap();

        writer.write_block(&block).unwrap();
        writer.finish().unwrap();

        assert_eq!(output, b"shot L0\nshot L1\n");
    }

    #[test]
    fn writes_non_dets_detector_output_without_observables() {
        let block = decoded_block(
            BitTable::new(0, 2),
            table_from_shots(&[&[true, false], &[false, true]]),
            table_from_shots(&[&[true, true], &[true, true]]),
        );
        let mut output = Vec::new();
        let mut writer = ResultBlockWriter::new(
            &mut output,
            ResultOutputKind::Detectors,
            OutputFormat::Format01,
        )
        .unwrap();

        writer.write_block(&block).unwrap();
        writer.finish().unwrap();

        assert_eq!(output, b"10\n01\n");
    }

    #[test]
    fn rejects_mismatched_shot_counts_before_writing_a_block() {
        let block = decoded_block(
            BitTable::new(1, 2),
            BitTable::new(1, 1),
            BitTable::new(1, 2),
        );
        let sentinel = b"unchanged".to_vec();
        let mut output = sentinel.clone();
        let mut writer = ResultBlockWriter::new(
            &mut output,
            ResultOutputKind::Measurements,
            OutputFormat::Format01,
        )
        .unwrap();

        assert!(writer.write_block(&block).is_err());
        drop(writer);

        assert_eq!(output, sentinel);
    }

    #[test]
    fn carries_ptb64_groups_across_decoded_blocks_until_finish() {
        let first = decoded_block(
            sample_table(3, 40),
            BitTable::new(0, 40),
            BitTable::new(0, 40),
        );
        let second = decoded_block(
            sample_table_with_shot_offset(3, 25, 40),
            BitTable::new(0, 25),
            BitTable::new(0, 25),
        );
        let expected = join_tables(&first.measurements, &second.measurements);
        let mut expected_output = Vec::new();
        write_shots_ptb64(&expected, &mut expected_output).unwrap();
        let mut output = Vec::new();
        let mut writer = ResultBlockWriter::new(
            &mut output,
            ResultOutputKind::Measurements,
            OutputFormat::Ptb64,
        )
        .unwrap();

        writer.write_block(&first).unwrap();
        writer.write_block(&second).unwrap();
        writer.finish().unwrap();

        assert_eq!(output, expected_output);
    }

    #[test]
    fn rejects_dets_for_measurements_and_observables() {
        for kind in [
            ResultOutputKind::Measurements,
            ResultOutputKind::Observables,
        ] {
            assert!(ResultBlockWriter::new(Vec::<u8>::new(), kind, OutputFormat::Dets).is_err());
        }
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
        sample_table_with_shot_offset(width, shots, 0)
    }

    fn sample_table_with_shot_offset(width: usize, shots: usize, shot_offset: usize) -> BitTable {
        let mut table = BitTable::new(width, shots);
        for bit in 0..width {
            for shot in 0..shots {
                table.set(bit, shot, (bit * 17 + (shot + shot_offset) * 11) % 5 < 2);
            }
        }
        table
    }

    fn decoded_block(
        measurements: BitTable,
        detections: BitTable,
        observable_flips: BitTable,
    ) -> DecodedSampleBlock {
        DecodedSampleBlock {
            measurements,
            detections,
            observable_flips,
        }
    }

    fn table_from_shots(shots: &[&[bool]]) -> BitTable {
        let width = shots.first().map_or(0, |shot| shot.len());
        let mut table = BitTable::new(width, shots.len());
        for (shot_index, shot) in shots.iter().enumerate() {
            assert_eq!(shot.len(), width);
            for (bit, &value) in shot.iter().enumerate() {
                table.set(bit, shot_index, value);
            }
        }
        table
    }

    fn join_tables(first: &BitTable, second: &BitTable) -> BitTable {
        assert_eq!(first.num_major(), second.num_major());
        let mut joined = BitTable::new(first.num_major(), first.num_minor() + second.num_minor());
        for bit in 0..joined.num_major() {
            for shot in 0..first.num_minor() {
                joined.set(bit, shot, first.get(bit, shot));
            }
            for shot in 0..second.num_minor() {
                joined.set(bit, first.num_minor() + shot, second.get(bit, shot));
            }
        }
        joined
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
