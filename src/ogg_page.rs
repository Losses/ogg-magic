use std::fmt;
use std::io::Cursor;
use std::error::Error;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::crc;
use crc::vorbis_crc32;


const OGG_MAGIC_SIGNATURE: [u8; 4] = [0x4f, 0x67, 0x67, 0x53];
const FOUR_ZERO_BYTES: [u8; 4] = [0, 0, 0, 0];

/// Error type representing issues with Ogg format.
#[derive(Debug)]
pub struct OggFormatError(String);

impl fmt::Display for OggFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OggFormatError: {}", self.0)
    }
}

impl Error for OggFormatError {}

/// Represents an Ogg page with various metadata and methods to manipulate it.
#[derive(Debug, Clone)]
pub struct OggPage {
    pub ready: bool,
    pub buffer: Vec<u8>,
    pub structure_version: u8,
    pub header_type: u8,
    pub is_fresh_packet: bool,
    pub is_bos: bool,
    pub is_boe: bool,
    pub absolute_granule_position: u64,
    pub stream_serial_number: u32,
    pub page_sequence_number: u32,
    pub page_checksum: u32,
    pub page_segments: usize,
    pub segment_table: Vec<u8>,
    pub parsed_segment_table: Vec<usize>,
    pub page_size: usize,
}

impl OggPage {
    /// Creates a new `OggPage` from a buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A vector of bytes representing the Ogg page.
    ///
    /// # Returns
    ///
    /// * `Result<Self, OggFormatError>` - An instance of `OggPage` or an error if the buffer is invalid.
    pub fn new(buffer: Vec<u8>) -> Result<Self, OggFormatError> {
        let (page_size, page_segments_count) = Self::validate_page(&buffer)?;

        let mut cursor = Cursor::new(&buffer);
        cursor.set_position(4);
        let structure_version = cursor.read_u8().unwrap();
        let header_type = cursor.read_u8().unwrap();
        let is_fresh_packet = (header_type & 0x1) == 0;
        let is_bos = (header_type & 0x2) != 0;
        let is_boe = (header_type & 0x4) != 0;
        cursor.set_position(6);
        let absolute_granule_position = cursor.read_u64::<LittleEndian>().unwrap();
        let stream_serial_number = cursor.read_u32::<LittleEndian>().unwrap();
        let page_sequence_number = cursor.read_u32::<LittleEndian>().unwrap();
        let page_checksum = cursor.read_u32::<LittleEndian>().unwrap();
        let segment_table = buffer[27..27 + page_segments_count].to_vec();

        let mut parsed_segment_table = Vec::new();
        let mut accumulated_size = 0;
        for &segment_size in &segment_table {
            if segment_size == 255 {
                accumulated_size += segment_size as usize;
                continue;
            }
            if accumulated_size > 0 {
                accumulated_size += segment_size as usize;
                parsed_segment_table.push(accumulated_size);
                accumulated_size = 0;
                continue;
            }
            if segment_size != 0 {
                parsed_segment_table.push(segment_size as usize);
            }
        }

        let page_segments = parsed_segment_table.len();

        Ok(Self {
            ready: false,
            buffer: buffer[..page_size].to_vec(),
            structure_version,
            header_type,
            is_fresh_packet,
            is_bos,
            is_boe,
            absolute_granule_position,
            stream_serial_number,
            page_sequence_number,
            page_checksum,
            page_segments,
            segment_table,
            parsed_segment_table,
            page_size,
        })
    }

    /// Validates the Ogg page from the buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A slice of bytes representing the Ogg page.
    ///
    /// # Returns
    ///
    /// * `Result<(usize, usize), OggFormatError>` - A tuple containing the page size and segment count or an error if the buffer is invalid.
    fn validate_page(buffer: &[u8]) -> Result<(usize, usize), OggFormatError> {
        if buffer.len() < 27 {
            return Err(OggFormatError("Incomplete buffer length".to_string()));
        }

        for i in 0..OGG_MAGIC_SIGNATURE.len() {
            if buffer[i] != OGG_MAGIC_SIGNATURE[i] {
                return Err(OggFormatError(format!(
                    "Invalid OGG magic signature at position {}, got {} but expected {}",
                    i, buffer[i], OGG_MAGIC_SIGNATURE[i]
                )));
            }
        }

        let page_segments_count = buffer[26] as usize;
        if buffer.len() < 27 + page_segments_count {
            return Err(OggFormatError("Incomplete segment table".to_string()));
        }

        let mut page_body_size = 0;
        for i in 0..page_segments_count {
            page_body_size += buffer[27 + i] as usize;
        }

        let page_size = 27 + page_segments_count + page_body_size;
        if buffer.len() < page_size {
            return Err(OggFormatError(format!(
                "Insufficient page size, expected {}, at least {}",
                page_size,
                buffer.len()
            )));
        }

        Ok((page_size, page_segments_count))
    }

    /// Validates the size of the Ogg page.
    ///
    /// # Returns
    ///
    /// * `Result<(), OggFormatError>` - Ok if the size is valid, otherwise an error.
    pub fn validate_page_size(&self) -> Result<(), OggFormatError> {
        let header_size = self.segment_table.len() + 27;
        let page_body_size: usize = self.segment_table.iter().map(|&x| x as usize).sum();
        let page_size = header_size + page_body_size;

        if page_size != self.buffer.len() {
            return Err(OggFormatError(format!(
                "Invalid page size, expected {} bytes but actually {} bytes.",
                page_size,
                self.buffer.len()
            )));
        }

        Ok(())
    }

    /// Retrieves a specific segment from the Ogg page.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the segment to retrieve.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<u8>, OggFormatError>` - The segment as a vector of bytes or an error if the index is out of range.
    pub fn get_page_segment(&self, index: usize) -> Result<Vec<u8>, OggFormatError> {
        if index >= self.parsed_segment_table.len() {
            return Err(OggFormatError(format!(
                "Segment start index out of range, index: {}, range: {}",
                index,
                self.parsed_segment_table.len()
            )));
        }
        let accumulated_page_segment_size: usize = self.parsed_segment_table[..index].iter().sum();
        let segment_length = self.parsed_segment_table[index];

        Ok(
            self.buffer[27 + self.segment_table.len() + accumulated_page_segment_size
                ..27 + self.segment_table.len() + accumulated_page_segment_size + segment_length]
                .to_vec(),
        )
    }

    /// Applies a callback function to each segment of the Ogg page.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function to apply to each segment.
    ///
    /// # Returns
    ///
    /// * `Vec<T>` - A vector containing the results of applying the callback to each segment.
    pub fn map_segments<T, F>(&self, callback: F) -> Vec<T>
    where
        F: Fn(&[u8], usize) -> T,
    {
        let parsed_segment_length = self.parsed_segment_table.len();
        let mut result = Vec::with_capacity(parsed_segment_length);

        for i in 0..parsed_segment_length {
            result.push(callback(&self.get_page_segment(i).unwrap(), i));
        }

        result
    }

    /// Creates a laced vector from a slice of segment lengths.
    ///
    /// # Arguments
    ///
    /// * `input` - A slice of segment lengths.
    ///
    /// # Returns
    ///
    /// * `Vec<u8>` - A vector of bytes representing the laced segments.
    pub fn create_laced_vec(input: &[usize]) -> Vec<u8> {
        let mut result = Vec::new();
    
        for mut value in input.iter().cloned() {
            while value >= 255 {
                result.push(255);
                value -= 255;
            }
            result.push(value as u8);
        }
    
        if *result.last().unwrap() == 255 {
            result.push(0);
        }
    
        result
    }

    /// Removes a segment from the Ogg page and returns the raw result.
    ///
    /// # Arguments
    ///
    /// * `index` - The starting index of the segment to remove.
    /// * `n` - The number of segments to remove.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<u8>, OggFormatError>` - The new buffer as a vector of bytes or an error if the operation fails.
    pub fn remove_page_segment_and_get_raw_result(
        &self,
        index: usize,
        n: usize,
    ) -> Result<Vec<u8>, OggFormatError> {
        if index >= self.parsed_segment_table.len() {
            return Err(OggFormatError(format!(
                "Segment start index out of range, index: {}, range: {}",
                index,
                self.parsed_segment_table.len()
            )));
        }
        if n == 0 {
            return Err(OggFormatError(
                "Number of segments to remove must be greater than 0".to_string(),
            ));
        }
        if index + n > self.parsed_segment_table.len() {
            return Err(OggFormatError(format!(
                "Segment end index out of range, index: {}, range: {}",
                index + n,
                self.parsed_segment_table.len()
            )));
        }

        let accumulated_page_segment_size: usize = self.parsed_segment_table[..index].iter().sum();
        let segments_to_remove: Vec<usize> = self.parsed_segment_table[index..index + n].to_vec();
        let total_remove_length: usize = segments_to_remove.iter().sum();

        let mut new_parsed_segment_table = self.parsed_segment_table.clone();
        new_parsed_segment_table.drain(index..index + n);

        let new_segment_table = Self::create_laced_vec(&new_parsed_segment_table);
        let new_segments = new_segment_table.len();

        let new_buffer_size = self.buffer.len() - total_remove_length - self.segment_table.len()
            + new_segment_table.len();
        let mut new_buffer = vec![0; new_buffer_size];

        new_buffer[..26].copy_from_slice(&self.buffer[..26]);
        new_buffer[26] = new_segment_table.len() as u8;
        new_buffer[27..27 + new_segment_table.len()].copy_from_slice(&new_segment_table);

        let remove_from = 27 + self.segment_table.len() + accumulated_page_segment_size;
        let remove_to = remove_from + total_remove_length;

        new_buffer[27 + new_segments
            ..27 + new_segments + (remove_from - (27 + self.segment_table.len()))]
            .copy_from_slice(&self.buffer[27 + self.segment_table.len()..remove_from]);

        new_buffer[27 + new_segments + (remove_from - (27 + self.segment_table.len()))..]
            .copy_from_slice(&self.buffer[remove_to..]);

        if new_buffer.len() != new_buffer_size {
            return Err(OggFormatError(format!(
                "Calculated new buffer size {} does not match actual size {}",
                new_buffer_size,
                new_buffer.len()
            )));
        }

        Ok(new_buffer)
    }

    /// Adds segments to the Ogg page and returns the raw result.
    ///
    /// # Arguments
    ///
    /// * `segments` - A slice of vectors, each representing a segment to add.
    /// * `index` - The index at which to add the segments.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<u8>, OggFormatError>` - The new buffer as a vector of bytes or an error if the operation fails.
    pub fn add_page_segment_and_get_raw_result(
        &self,
        segments: &[Vec<u8>],
        index: usize,
    ) -> Result<Vec<u8>, OggFormatError> {
        if index > self.parsed_segment_table.len() {
            return Err(OggFormatError(format!(
                "Segment start index out of range, index: {}, range: {}",
                index,
                self.parsed_segment_table.len()
            )));
        }

        let new_segments_lengths: Vec<usize> =
            segments.iter().map(|segment| segment.len()).collect();
        let mut new_parsed_segment_table = Vec::new();
        new_parsed_segment_table.extend_from_slice(&self.parsed_segment_table[..index]);
        new_parsed_segment_table.extend_from_slice(&new_segments_lengths);
        new_parsed_segment_table.extend_from_slice(&self.parsed_segment_table[index..]);

        let new_segment_table = Self::create_laced_vec(&new_parsed_segment_table);
        let new_segments = new_segment_table.len();

        let total_new_length: usize = new_segments_lengths.iter().sum();
        let new_buffer_size = self.buffer.len() + total_new_length + new_segment_table.len()
            - self.segment_table.len();
        let mut new_buffer = vec![0; new_buffer_size];

        new_buffer[..26].copy_from_slice(&self.buffer[..26]);
        new_buffer[26] = new_segment_table.len() as u8;
        new_buffer[27..27 + new_segment_table.len()].copy_from_slice(&new_segment_table);

        let accumulated_page_segment_size: usize = self.parsed_segment_table[..index].iter().sum();
        let insert_at = 27 + self.segment_table.len() + accumulated_page_segment_size;

        new_buffer
            [27 + new_segments..27 + new_segments + (insert_at - (27 + self.segment_table.len()))]
            .copy_from_slice(&self.buffer[27 + self.segment_table.len()..insert_at]);

        let mut offset = 27 + new_segments + (insert_at - (27 + self.segment_table.len()));
        for segment in segments {
            new_buffer[offset..offset + segment.len()].copy_from_slice(segment);
            offset += segment.len();
        }

        new_buffer[offset..].copy_from_slice(&self.buffer[insert_at..]);

        if new_buffer.len() != new_buffer_size {
            return Err(OggFormatError(format!(
                "Calculated new buffer size {} does not match actual size {}",
                new_buffer_size,
                new_buffer.len()
            )));
        }

        Ok(new_buffer)
    }

    /// Replaces a segment in the Ogg page and returns the raw result.
    ///
    /// # Arguments
    ///
    /// * `segment` - A vector of bytes representing the new segment.
    /// * `index` - The index of the segment to replace.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<u8>, OggFormatError>` - The new buffer as a vector of bytes or an error if the operation fails.
    pub fn replace_page_segment_and_get_raw_result(
        &self,
        segment: Vec<u8>,
        index: usize,
    ) -> Result<Vec<u8>, OggFormatError> {
        if index >= self.parsed_segment_table.len() {
            return Err(OggFormatError(format!(
                "Segment start index out of range, index: {}, range: {}",
                index,
                self.parsed_segment_table.len()
            )));
        }

        let old_segment_length = self.parsed_segment_table[index];
        let new_segment_length = segment.len();

        let mut new_parsed_segment_table = self.parsed_segment_table.clone();
        new_parsed_segment_table[index] = new_segment_length;

        let new_segment_table = Self::create_laced_vec(&new_parsed_segment_table);
        let new_segments = new_segment_table.len();

        let new_buffer_size =
            self.buffer.len() - old_segment_length + new_segment_length + new_segment_table.len()
                - self.segment_table.len();
        let mut new_buffer = vec![0; new_buffer_size];

        new_buffer[..26].copy_from_slice(&self.buffer[..26]);
        new_buffer[26] = new_segment_table.len() as u8;
        new_buffer[27..27 + new_segment_table.len()].copy_from_slice(&new_segment_table);

        let accumulated_page_segment_size: usize = self.parsed_segment_table[..index].iter().sum();
        let replace_at = 27 + self.segment_table.len() + accumulated_page_segment_size;
        let replace_end = replace_at + old_segment_length;

        new_buffer
            [27 + new_segments..27 + new_segments + (replace_at - (27 + self.segment_table.len()))]
            .copy_from_slice(&self.buffer[27 + self.segment_table.len()..replace_at]);

        new_buffer[27 + new_segments + (replace_at - (27 + self.segment_table.len()))
            ..27 + new_segments
                + (replace_at - (27 + self.segment_table.len()))
                + new_segment_length]
            .copy_from_slice(&segment);

        new_buffer[27
            + new_segments
            + (replace_at - (27 + self.segment_table.len()))
            + new_segment_length..]
            .copy_from_slice(&self.buffer[replace_end..]);

        if new_buffer.len() != new_buffer_size {
            return Err(OggFormatError(format!(
                "Calculated new buffer size {} does not match actual size {}",
                new_buffer_size,
                new_buffer.len()
            )));
        }

        Ok(new_buffer)
    }

    /// Removes a segment from the Ogg page.
    ///
    /// # Arguments
    ///
    /// * `index` - The starting index of the segment to remove.
    /// * `n` - The number of segments to remove.
    ///
    /// # Returns
    ///
    /// * `Result<Self, OggFormatError>` - A new `OggPage` instance or an error if the operation fails.
    pub fn remove_page_segment(&self, index: usize, n: usize) -> Result<Self, OggFormatError> {
        let new_buffer = self.remove_page_segment_and_get_raw_result(index, n)?;
        let mut new_page = Self::new(new_buffer)?;
        new_page.update_page_checksum();
        Ok(new_page)
    }

    /// Adds segments to the Ogg page.
    ///
    /// # Arguments
    ///
    /// * `segments` - A slice of vectors, each representing a segment to add.
    /// * `index` - The index at which to add the segments.
    ///
    /// # Returns
    ///
    /// * `Result<Self, OggFormatError>` - A new `OggPage` instance or an error if the operation fails.
    pub fn add_page_segment(
        &self,
        segments: &[Vec<u8>],
        index: usize,
    ) -> Result<Self, OggFormatError> {
        let new_buffer = self.add_page_segment_and_get_raw_result(segments, index)?;
        let mut new_page = Self::new(new_buffer)?;
        new_page.update_page_checksum();
        Ok(new_page)
    }

    /// Replaces a segment in the Ogg page.
    ///
    /// # Arguments
    ///
    /// * `segment` - A vector of bytes representing the new segment.
    /// * `index` - The index of the segment to replace.
    ///
    /// # Returns
    ///
    /// * `Result<Self, OggFormatError>` - A new `OggPage` instance or an error if the operation fails.
    pub fn replace_page_segment(
        &self,
        segment: Vec<u8>,
        index: usize,
    ) -> Result<Self, OggFormatError> {
        let new_buffer = self.replace_page_segment_and_get_raw_result(segment, index)?;
        let mut new_page = Self::new(new_buffer)?;
        new_page.update_page_checksum();
        Ok(new_page)
    }

    /// Calculates the checksum of the Ogg page.
    ///
    /// # Returns
    ///
    /// * `u32` - The calculated checksum.
    pub fn calculate_page_checksum(&self) -> u32 {
        let mut calculated_checksum = 0;

        // Update CRC32 with the header part before the checksum field
        calculated_checksum = vorbis_crc32(&self.buffer, calculated_checksum, 0, 22);

        // Update CRC32 with four zero bytes (the checksum field)
        calculated_checksum = vorbis_crc32(
            &FOUR_ZERO_BYTES,
            calculated_checksum,
            0,
            FOUR_ZERO_BYTES.len(),
        );

        // Update CRC32 with the rest of the buffer after the checksum field
        calculated_checksum =
            vorbis_crc32(&self.buffer, calculated_checksum, 26, self.buffer.len());

        calculated_checksum
    }

    /// Updates the checksum of the Ogg page.
    pub fn update_page_checksum(&mut self) {
        let page_checksum = self.calculate_page_checksum();
        let mut cursor = Cursor::new(&mut self.buffer[22..26]);
        cursor.write_u32::<LittleEndian>(page_checksum).unwrap();
        self.page_checksum = page_checksum;
    }

    /// Checks if the checksum of the Ogg page is correct.
    ///
    /// # Returns
    ///
    /// * `bool` - `true` if the checksum is correct, `false` otherwise.
    pub fn if_page_checksum_correct(&self) -> bool {
        self.calculate_page_checksum() == self.page_checksum
    }
}
