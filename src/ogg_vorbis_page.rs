use lazy_static::lazy_static;
use num_traits::PrimInt;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str;

use crate::ogg_page::OggPage;

const VORBIS_HEAD_MAGIC_SIGNATURE: [u8; 6] = [0x76, 0x6f, 0x72, 0x62, 0x69, 0x73];
const VORBIS_SETUP_CODEBOOK_MAGIC_SIGNATURE: [u8; 3] = [0x42, 0x43, 0x56];

lazy_static! {
    static ref ALLOWED_BLOCK_SIZES: HashSet<usize> = {
        let mut set = HashSet::new();
        set.insert(64);
        set.insert(128);
        set.insert(256);
        set.insert(512);
        set.insert(1024);
        set.insert(2048);
        set.insert(4096);
        set.insert(8192);
        set
    };
}

/// Represents an error related to the Vorbis format.
#[derive(Debug)]
pub struct VorbisFormatError(String);

/// A reader for reading bits from a byte array.
impl fmt::Display for VorbisFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VorbisFormatError: {}", self.0)
    }
}

impl Error for VorbisFormatError {}

pub struct BitStreamReader {
    data: Vec<u8>,
    cursor: usize,
}

impl BitStreamReader {
    /// Creates a new `BitStreamReader` with the given data and offset.
    pub fn new(data: Vec<u8>, offset: usize) -> Self {
        Self {
            data,
            cursor: offset,
        }
    }

    /// Reads a single bit from the stream.
    fn read_bit(&mut self) -> u8 {
        let byte_index = self.cursor / 8;
        let bit_index = self.cursor % 8;
        let bit = (self.data[byte_index] >> bit_index) & 1;
        self.cursor += 1;
        bit
    }

    /// Reads a specified number of bits from the stream as a number.
    fn read_bits_as_number(&mut self, num_bits: usize) -> u32 {
        let mut result = 0;
        for i in 0..num_bits {
            let bit = self.read_bit();
            result |= (bit as u32) << i;
        }
        result
    }

    /// Reads a boolean value from the stream.
    pub fn read_bool(&mut self) -> bool {
        self.read_bits_as_number(1) == 1
    }

    /// Reads an unsigned integer of `x` bits from the stream.
    pub fn read_uint_n(&mut self, x: usize) -> u32 {
        self.read_bits_as_number(x)
    }

    pub fn read_uint2(&mut self) -> u8 {
        self.read_bits_as_number(2) as u8
    }

    pub fn read_uint3(&mut self) -> u8 {
        self.read_bits_as_number(3) as u8
    }

    pub fn read_uint4(&mut self) -> u8 {
        self.read_bits_as_number(4) as u8
    }

    pub fn read_uint5(&mut self) -> u8 {
        self.read_bits_as_number(5) as u8
    }

    pub fn read_uint6(&mut self) -> u8 {
        self.read_bits_as_number(6) as u8
    }

    pub fn read_uint8(&mut self) -> u8 {
        self.read_bits_as_number(8) as u8
    }

    pub fn read_uint16(&mut self) -> u16 {
        self.read_bits_as_number(16) as u16
    }

    pub fn read_uint24(&mut self) -> u32 {
        self.read_bits_as_number(24)
    }

    pub fn read_uint32(&mut self) -> u32 {
        self.read_bits_as_number(32)
    }
}

/// Unpacks a 32-bit float value from a given 32-bit integer.
pub fn float32_unpack(x: u32) -> f32 {
    let mantissa = x & 0x1fffff;
    let sign = x & 0x80000000;
    let exponent = (x & 0x7fe00000) >> 21;
    let signed_mantissa = if sign != 0 {
        -(mantissa as i32)
    } else {
        mantissa as i32
    };
    signed_mantissa as f32 * 2.0f32.powi(exponent as i32 - 788)
}

/// Calculates the number of values for a lookup table with the given entries and dimensions.
pub fn lookup1_values(entries: u32, dimensions: u32) -> u32 {
    let mut low = 1;
    let mut high = entries;

    while low < high {
        let mid = (low + high + 1) / 2;
        let mid_pow = (mid as f64).powi(dimensions as i32);
        if mid_pow <= entries as f64 {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    low
}

/// Represents an Ogg Vorbis page.
#[derive(Debug, Clone)]
pub struct OggVorbisPage {
    pub ogg_page: OggPage,
}

impl OggVorbisPage {
    /// Creates a new `OggVorbisPage` from the given buffer.
    pub fn new(buffer: Vec<u8>) -> Result<Self, VorbisFormatError> {
        let ogg_page = OggPage::new(buffer).map_err(|e| VorbisFormatError(e.to_string()))?;
        Ok(Self { ogg_page })
    }

    /// Retrieves the identification header from the specified segment index.
    pub fn get_identification(
        &self,
        segment_index: usize,
    ) -> Result<IVorbisIdentificationHeader, VorbisFormatError> {
        let array = self
            .ogg_page
            .get_page_segment(segment_index)
            .map_err(|e| VorbisFormatError(e.to_string()))?;

        if !self.is_header_packet(segment_index) {
            return Err(VorbisFormatError("Invalid magic signature".to_string()));
        }

        if !self.is_identification_packet(segment_index) {
            return Err(VorbisFormatError(
                "The packet is not an identification packet".to_string(),
            ));
        }

        let mut reader = BitStreamReader::new(array, 7 * 8);

        let vorbis_version = reader.read_uint32();
        if vorbis_version != 0 {
            return Err(VorbisFormatError(format!(
                "Unsupported Vorbis version: {}",
                vorbis_version
            )));
        }

        let audio_channels = reader.read_uint8();

        let audio_sample_rate = reader.read_uint32();

        let bitrate_maximum = reader.read_uint32();
        let bitrate_nominal = reader.read_uint32();
        let bitrate_minimum = reader.read_uint32();

        let blocksize0 = 1 << reader.read_uint4();
        let blocksize1 = 1 << reader.read_uint4();

        if !ALLOWED_BLOCK_SIZES.contains(&blocksize0) {
            return Err(VorbisFormatError(format!(
                "Invalid blocksize0 values: {}",
                blocksize0
            )));
        }

        if !ALLOWED_BLOCK_SIZES.contains(&blocksize1) {
            return Err(VorbisFormatError(format!(
                "Invalid blocksize1 values: {}",
                blocksize1
            )));
        }

        if blocksize0 > blocksize1 {
            return Err(VorbisFormatError(
                "blocksize0 must be less than or equal to blocksize1".to_string(),
            ));
        }

        let framing_flag = reader.read_bool();
        if !framing_flag {
            return Err(VorbisFormatError("Framing bit must be nonzero".to_string()));
        }

        Ok(IVorbisIdentificationHeader {
            vorbis_version,
            audio_channels,
            audio_sample_rate,
            bitrate_maximum,
            bitrate_nominal,
            bitrate_minimum,
            blocksize0,
            blocksize1,
            framing_flag,
        })
    }

    /// Checks if the specified segment is a header packet.
    pub fn is_header_packet(&self, segment_index: usize) -> bool {
        let array = self.ogg_page.get_page_segment(segment_index).unwrap();
        for i in 0..VORBIS_HEAD_MAGIC_SIGNATURE.len() {
            if array[i + 1] != VORBIS_HEAD_MAGIC_SIGNATURE[i] {
                return false;
            }
        }
        true
    }

    /// Checks if the specified segment is an identification packet.
    pub fn is_identification_packet(&self, segment_index: usize) -> bool {
        let array = self.ogg_page.get_page_segment(segment_index).unwrap();
        array[0] == VorbisHeaderType::Identification as u8
    }

    /// Checks if the specified segment is a comment packet.
    pub fn is_comment_packet(&self, segment_index: usize) -> bool {
        let array = self.ogg_page.get_page_segment(segment_index).unwrap();
        array[0] == VorbisHeaderType::Comment as u8
    }

    /// Checks if the specified segment is a setup packet.
    pub fn is_setup_packet(&self, segment_index: usize) -> bool {
        let array = self.ogg_page.get_page_segment(segment_index).unwrap();
        array[0] == VorbisHeaderType::Setup as u8
    }

    /// Retrieves the comments from the specified segment index.
    pub fn get_comments(
        &self,
        segment_index: usize,
    ) -> Result<IVorbisCommentHeader, VorbisFormatError> {
        let array = self
            .ogg_page
            .get_page_segment(segment_index)
            .map_err(|e| VorbisFormatError(e.to_string()))?;

        if !self.is_header_packet(segment_index) {
            return Err(VorbisFormatError("Invalid magic signature".to_string()));
        }

        if !self.is_comment_packet(segment_index) {
            return Err(VorbisFormatError(
                "The packet is not a comment packet".to_string(),
            ));
        }

        let mut reader = BitStreamReader::new(array.clone(), 7 * 8);

        let vendor_length = reader.read_uint32();
        let vendor_array =
            &array[(reader.cursor / 8)..((reader.cursor / 8) + vendor_length as usize)];
        let vendor = str::from_utf8(vendor_array)
            .map_err(|_| VorbisFormatError("Invalid UTF-8 sequence".to_string()))?
            .to_string();
        reader.cursor += vendor_length as usize * 8;

        let user_comment_list_length = reader.read_uint32();

        let mut comments = std::collections::HashMap::new();
        for _ in 0..user_comment_list_length {
            let comment_length = reader.read_uint32();
            let comment_array =
                &array[(reader.cursor / 8)..((reader.cursor / 8) + comment_length as usize)];
            let comment = str::from_utf8(comment_array)
                .map_err(|_| VorbisFormatError("Invalid UTF-8 sequence".to_string()))?
                .to_string();
            reader.cursor += comment_length as usize * 8;

            let parts: Vec<&str> = comment.splitn(2, '=').collect();
            if parts.len() == 2 {
                let field_name = parts[0].to_uppercase();
                let field_value = parts[1].to_string();
                comments
                    .entry(field_name)
                    .or_insert_with(Vec::new)
                    .push(field_value);
            }
        }

        let framing_bit = reader.read_bool();
        if !framing_bit {
            return Err(VorbisFormatError("Framing bit must be nonzero".to_string()));
        }

        Ok(IVorbisCommentHeader { vendor, comments })
    }

    /// Parses a setup codebook from the given `BitStreamReader`.
    fn parse_setup_codebook(
        reader: &mut BitStreamReader,
    ) -> Result<IVorbisSetupCodebook, VorbisFormatError> {
        for &byte in &VORBIS_SETUP_CODEBOOK_MAGIC_SIGNATURE {
            let read_byte = reader.read_uint8();
            if read_byte != byte {
                return Err(VorbisFormatError(format!(
                    "Invalid codebook magic string, expected {:#x}, got {:#x} in position {}",
                    byte,
                    read_byte,
                    reader.cursor - 8
                )));
            }
        }

        let dimensions = reader.read_uint16();
        let entries = reader.read_uint24();

        let ordered = reader.read_bool();

        let mut codeword_lengths = vec![0; entries as usize];

        if ordered {
            let mut current_entry = 0;
            let mut current_length = reader.read_uint5() + 1;

            while current_entry < entries {
                let number = reader.read_uint_n(ilog(entries - current_entry) as usize);
                if current_entry + number > entries {
                    return Err(VorbisFormatError(
                        "Invalid codebook: too many codewords".to_string(),
                    ));
                }
                for _ in 0..number {
                    codeword_lengths[current_entry as usize] = current_length;
                    current_entry += 1;
                }
                current_length += 1;
            }
        } else {
            let sparse = reader.read_bool();
            for i in 0..entries {
                if sparse {
                    let flag = reader.read_bool();
                    if flag {
                        codeword_lengths[i as usize] = reader.read_uint5() + 1;
                    } else {
                        codeword_lengths[i as usize] = 0;
                    }
                } else {
                    codeword_lengths[i as usize] = reader.read_uint5() + 1;
                }
            }
        }

        let lookup_type: VorbisSetupCodebookLookupType = reader.read_uint4().into();

        let mut minimum_value = None;
        let mut delta_value = None;
        let mut value_bits = None;
        let mut sequence_p = None;
        let mut multiplicands = None;

        if lookup_type == VorbisSetupCodebookLookupType::Implicitly
            || lookup_type == VorbisSetupCodebookLookupType::Explicitly
        {
            minimum_value = Some(float32_unpack(reader.read_uint32()));
            delta_value = Some(float32_unpack(reader.read_uint32()));
            value_bits = Some(reader.read_uint4() + 1);
            sequence_p = Some(reader.read_bool());

            let lookup_values = if lookup_type == VorbisSetupCodebookLookupType::Implicitly {
                lookup1_values(entries, dimensions as u32)
            } else {
                entries * (dimensions as u32)
            };

            multiplicands = Some(
                (0..lookup_values)
                    .map(|_| reader.read_uint_n(value_bits.unwrap() as usize))
                    .collect(),
            );
        } else if lookup_type > VorbisSetupCodebookLookupType::Explicitly {
            return Err(VorbisFormatError("Unsupported lookup type".to_string()));
        }

        Ok(IVorbisSetupCodebook {
            dimensions,
            entries,
            codeword_lengths,
            lookup_type,
            minimum_value,
            delta_value,
            value_bits,
            sequence_p,
            multiplicands,
        })
    }

    /// Parses a floor type 0 from the given `BitStreamReader`.
    fn parse_floor_type0(
        reader: &mut BitStreamReader,
        codebook_count: u8,
    ) -> Result<IVorbisFloorType0, VorbisFormatError> {
        let order = reader.read_uint8();
        let rate = reader.read_uint16();
        let bark_map_size = reader.read_uint16();
        let amplitude_bits = reader.read_uint6();
        let amplitude_offset = reader.read_uint8();
        let number_of_books = reader.read_uint4() + 1;

        let mut book_list = Vec::new();
        for _ in 0..number_of_books {
            let book = reader.read_uint8();
            if book > codebook_count {
                return Err(VorbisFormatError(
                    "Invalid book number in floor0_book_list".to_string(),
                ));
            }
            book_list.push(book);
        }

        Ok(IVorbisFloorType0 {
            order,
            rate,
            bark_map_size,
            amplitude_bits,
            amplitude_offset,
            number_of_books,
            book_list,
        })
    }

    /// Parses a floor type 1 from the given `BitStreamReader`.
    fn parse_floor_type1(
        reader: &mut BitStreamReader,
    ) -> Result<IVorbisFloorType1, VorbisFormatError> {
        let partitions = reader.read_uint5();

        let mut partition_class_list = vec![0; partitions as usize];
        for i in 0..partitions {
            partition_class_list[i as usize] = reader.read_uint4();
        }

        let maximum_class = *partition_class_list.iter().max().unwrap();

        let mut class_dimensions = vec![0; (maximum_class + 1) as usize];
        let mut class_subclasses = vec![0; (maximum_class + 1) as usize];
        let mut class_masterbooks = vec![-1; (maximum_class + 1) as usize];
        let mut subclass_books = vec![vec![-1; 8]; (maximum_class + 1) as usize];

        for i in 0..=maximum_class {
            class_dimensions[i as usize] = reader.read_uint3() + 1;
            class_subclasses[i as usize] = reader.read_uint2();
            if class_subclasses[i as usize] > 0 {
                class_masterbooks[i as usize] = reader.read_uint8() as i32;
            }
            for j in 0..(1 << class_subclasses[i as usize]) {
                subclass_books[i as usize][j as usize] = reader.read_uint8() as i32 - 1;
            }
        }

        let multiplier = reader.read_uint2() + 1;
        let rangebits = reader.read_uint4();

        let mut x_list = vec![0, 1 << rangebits];
        let mut values = 2;

        for i in 0..partitions {
            let current_class_number = partition_class_list[i as usize];
            for _ in 0..class_dimensions[current_class_number as usize] {
                x_list.push(reader.read_uint_n(rangebits as usize) as i32);
                values += 1;
            }
        }

        let unique_x_list: std::collections::HashSet<_> = x_list.iter().cloned().collect();
        if unique_x_list.len() != x_list.len() {
            return Err(VorbisFormatError(
                "Non-unique X values in floor1_X_list".to_string(),
            ));
        }

        if x_list.len() > 65 {
            return Err(VorbisFormatError(
                "floor1_X_list length exceeds 65 elements".to_string(),
            ));
        }

        Ok(IVorbisFloorType1 {
            partitions,
            partition_class_list,
            class_dimensions,
            class_subclasses,
            class_masterbooks,
            subclass_books,
            multiplier,
            rangebits,
            x_list,
            values,
        })
    }

    /// Parses a residue from the given `BitStreamReader`.
    fn parse_residue(
        reader: &mut BitStreamReader,
        codebook_count: u8,
    ) -> Result<IVorbisResidue, VorbisFormatError> {
        let residue_type = reader.read_uint16();
        if residue_type > 2 {
            return Err(VorbisFormatError(format!(
                "Invalid residue type {}",
                residue_type
            )));
        }

        let begin = reader.read_uint24();
        let end = reader.read_uint24();
        let partition_size = reader.read_uint24() + 1;
        let classifications = reader.read_uint6() + 1;
        let classbook = reader.read_uint8();

        if classbook >= codebook_count {
            return Err(VorbisFormatError("Invalid classbook number".to_string()));
        }

        let mut cascade = vec![0; classifications as usize];
        for i in 0..classifications {
            let low_bits = reader.read_uint3();
            let bitflag = reader.read_bool();
            let high_bits = if bitflag { reader.read_uint5() } else { 0 };
            cascade[i as usize] = (high_bits << 3) + low_bits;
        }

        let mut books = vec![vec![-1; 8]; classifications as usize];
        for i in 0..classifications {
            for j in 0..8 {
                if (cascade[i as usize] & (1 << j)) != 0 {
                    let book = reader.read_uint8();
                    if book >= codebook_count {
                        return Err(VorbisFormatError(
                            "Invalid book number in residue_books".to_string(),
                        ));
                    }
                    books[i as usize][j as usize] = book as i32;
                }
            }
        }

        Ok(IVorbisResidue {
            residue_type,
            begin,
            end,
            partition_size,
            classifications,
            classbook,
            cascade,
            books,
        })
    }

    /// Parses a mapping from the given `BitStreamReader`.
    fn parse_mapping(
        reader: &mut BitStreamReader,
        audio_channels: u8,
        floor_count: u8,
        residue_count: u8,
    ) -> Result<IVorbisMapping, VorbisFormatError> {
        // Step 2a: Read the mapping type (16 bits)
        let mapping_type = reader.read_uint16();
        if mapping_type != 0 {
            return Err(VorbisFormatError("Unsupported mapping type".to_string()));
        }

        // Step 2c-i: Read 1 bit as a boolean flag for submaps
        let submaps_flag = reader.read_bool();
        let submaps = if submaps_flag {
            reader.read_uint4() + 1
        } else {
            1
        };

        // Step 2c-ii: Read 1 bit as a boolean flag for coupling steps
        let coupling_flag = reader.read_bool();
        let mut coupling_steps = 0;
        let mut magnitude = Vec::new();
        let mut angle = Vec::new();

        if coupling_flag {
            coupling_steps = reader.read_uint8() + 1;
            let coupling_bits = ilog(audio_channels - 1);

            for _ in 0..coupling_steps {
                let magnitude_val = reader.read_uint_n(coupling_bits as usize) as u8;
                let angle_val = reader.read_uint_n(coupling_bits as usize) as u8;
                if magnitude_val == angle_val
                    || magnitude_val >= audio_channels
                    || angle_val >= audio_channels
                {
                    return Err(VorbisFormatError(
                        "Invalid coupling channel numbers".to_string(),
                    ));
                }
                magnitude.push(magnitude_val);
                angle.push(angle_val);
            }
        }

        // Step 2c-iii: Read 2 bits reserved field; if nonzero, the stream is undecodable
        let reserved = reader.read_uint2();
        if reserved != 0 {
            return Err(VorbisFormatError(
                "Invalid reserved field in mapping".to_string(),
            ));
        }

        // Step 2c-iv: Read channel multiplex settings if submaps > 1
        let mut mux = Vec::new();
        if submaps > 1 {
            for _ in 0..audio_channels {
                let mux_value = reader.read_uint4();
                if mux_value >= submaps {
                    return Err(VorbisFormatError("Invalid multiplex value".to_string()));
                }
                mux.push(mux_value);
            }
        }

        // Step 2c-v: Read floor and residue numbers for each submap
        let mut submap_floors = Vec::new();
        let mut submap_residues = Vec::new();
        for _ in 0..submaps {
            reader.read_uint8(); // Discard 8 bits (unused time configuration placeholder)
            let floor_number = reader.read_uint8();
            if floor_number >= floor_count {
                return Err(VorbisFormatError("Invalid floor number".to_string()));
            }
            submap_floors.push(floor_number);
            let residue_number = reader.read_uint8();
            if residue_number >= residue_count {
                return Err(VorbisFormatError("Invalid residue number".to_string()));
            }
            submap_residues.push(residue_number);
        }

        Ok(IVorbisMapping {
            submaps,
            coupling_steps,
            magnitude,
            angle,
            mux,
            submap_floors,
            submap_residues,
        })
    }

    /// Parses a mode from the given `BitStreamReader`.
    fn parse_mode(reader: &mut BitStreamReader) -> Result<IVorbisMode, VorbisFormatError> {
        let blockflag = reader.read_bool();
        let windowtype = reader.read_uint16();
        let transformtype = reader.read_uint16();
        let mapping = reader.read_uint8();

        if windowtype != 0 || transformtype != 0 {
            return Err(VorbisFormatError(
                "Invalid windowtype or transformtype".to_string(),
            ));
        }

        Ok(IVorbisMode {
            blockflag,
            windowtype,
            transformtype,
            mapping,
        })
    }

    /// Retrieves the setup header from the specified segment index.
    pub fn get_setup(
        &self,
        audio_channels: u8,
        segment_index: usize,
    ) -> Result<IVorbisSetupHeader, VorbisFormatError> {
        let array = self
            .ogg_page
            .get_page_segment(segment_index)
            .map_err(|e| VorbisFormatError(e.to_string()))?;

        if !self.is_header_packet(segment_index) {
            return Err(VorbisFormatError("Invalid magic signature".to_string()));
        }

        if !self.is_setup_packet(segment_index) {
            return Err(VorbisFormatError(
                "The packet is not a setup packet".to_string(),
            ));
        }

        let mut reader = BitStreamReader::new(array, 7 * 8);

        let mut codebooks = Vec::new();
        let codebook_count = reader.read_uint8() + 1;
        for _ in 0..codebook_count {
            codebooks.push(Self::parse_setup_codebook(&mut reader)?);
        }

        let time_count = reader.read_uint6() + 1;
        for _ in 0..time_count {
            let time_type = reader.read_uint16();
            if time_type != 0 {
                return Err(VorbisFormatError("Invalid time type".to_string()));
            }
        }

        let floor_count = reader.read_uint6() + 1;
        let mut floors = Vec::new();
        for _ in 0..floor_count {
            let floor_type = reader.read_uint16();
            if floor_type == 0 {
                floors.push(IVorbisFloor::Type0(Self::parse_floor_type0(
                    &mut reader,
                    codebook_count,
                )?));
            } else if floor_type == 1 {
                floors.push(IVorbisFloor::Type1(Self::parse_floor_type1(&mut reader)?));
            } else {
                return Err(VorbisFormatError("Invalid floor type".to_string()));
            }
        }

        let residue_count = reader.read_uint6() + 1;
        let mut residues = Vec::new();
        for _ in 0..residue_count {
            residues.push(Self::parse_residue(&mut reader, codebook_count)?);
        }

        let mapping_count = reader.read_uint6() + 1;
        let mut mappings = Vec::new();
        for _ in 0..mapping_count {
            mappings.push(Self::parse_mapping(
                &mut reader,
                audio_channels,
                floor_count,
                residue_count,
            )?);
        }

        let mode_count = reader.read_uint6() + 1;
        let mut modes = Vec::new();
        for _ in 0..mode_count {
            modes.push(Self::parse_mode(&mut reader)?);
        }

        let framing_bit = reader.read_bool();
        if !framing_bit {
            return Err(VorbisFormatError("Framing bit must be nonzero".to_string()));
        }

        Ok(IVorbisSetupHeader {
            codebooks,
            floors,
            residues,
            mappings,
            modes,
            framing_bit,
        })
    }

    pub fn remove_page_segment(&self, index: usize, n: usize) -> Result<Self, VorbisFormatError> {
        let new_buffer = self
            .ogg_page
            .remove_page_segment_and_get_raw_result(index, n)
            .map_err(|e| VorbisFormatError(e.to_string()))?;
        let mut new_page = Self::new(new_buffer)?;
        new_page.update_page_checksum();
        Ok(new_page)
    }

    pub fn replace_page_segment(&self, segment: Vec<u8>, index: usize) -> Result<Self, VorbisFormatError> {
        let new_buffer = self.ogg_page.replace_page_segment_and_get_raw_result(segment, index)
            .map_err(|e| VorbisFormatError(e.to_string()))?;
        let mut new_page = Self::new(new_buffer)?;
        new_page.update_page_checksum();
        Ok(new_page)
    }

    pub fn build_comments(header: IVorbisCommentHeader) -> Vec<u8> {
        let mut result = Vec::new();
        result.push(3); // Comment packet type
        result.extend_from_slice(&VORBIS_HEAD_MAGIC_SIGNATURE);

        let mut vendor_bytes = header.vendor.into_bytes();
        result.extend_from_slice(&(vendor_bytes.len() as u32).to_le_bytes());
        result.append(&mut vendor_bytes);

        result.extend_from_slice(&(header.comments.len() as u32).to_le_bytes());
        for (key, values) in header.comments {
            for value in values {
                let comment = format!("{}={}", key, value);
                let mut comment_bytes = comment.into_bytes();
                result.extend_from_slice(&(comment_bytes.len() as u32).to_le_bytes());
                result.append(&mut comment_bytes);
            }
        }

        result.push(1); // Framing bit

        result
    }
}

/// Calculates the integer logarithm base 2 of a number.
pub fn ilog<T: PrimInt>(x: T) -> T {
    let bit_count = T::zero().count_zeros() as usize;
    let leading_zeros = x.leading_zeros() as usize;
    T::from(bit_count - leading_zeros).unwrap()
}

/// Represents the different types of Vorbis headers.
#[derive(Debug)]
pub enum VorbisHeaderType {
    Identification = 1,
    Comment = 3,
    Setup = 5,
}

/// Represents a Vorbis identification header.
#[derive(Debug)]
pub struct IVorbisIdentificationHeader {
    pub vorbis_version: u32,
    pub audio_channels: u8,
    pub audio_sample_rate: u32,
    pub bitrate_maximum: u32,
    pub bitrate_nominal: u32,
    pub bitrate_minimum: u32,
    pub blocksize0: usize,
    pub blocksize1: usize,
    pub framing_flag: bool,
}

/// Represents a Vorbis comment header.
#[derive(Debug)]
pub struct IVorbisCommentHeader {
    pub vendor: String,
    pub comments: std::collections::HashMap<String, Vec<String>>,
}

/// Represents a Vorbis setup header.
#[derive(Debug)]
pub struct IVorbisSetupHeader {
    pub codebooks: Vec<IVorbisSetupCodebook>,
    pub floors: Vec<IVorbisFloor>,
    pub residues: Vec<IVorbisResidue>,
    pub mappings: Vec<IVorbisMapping>,
    pub modes: Vec<IVorbisMode>,
    pub framing_bit: bool,
}

/// Represents a Vorbis setup codebook.
#[derive(Debug)]
pub struct IVorbisSetupCodebook {
    pub dimensions: u16,
    pub entries: u32,
    pub codeword_lengths: Vec<u8>,
    pub lookup_type: VorbisSetupCodebookLookupType,
    pub minimum_value: Option<f32>,
    pub delta_value: Option<f32>,
    pub value_bits: Option<u8>,
    pub sequence_p: Option<bool>,
    pub multiplicands: Option<Vec<u32>>,
}

/// Represents the lookup types for Vorbis setup codebooks.
#[derive(Debug, PartialEq, PartialOrd)]
pub enum VorbisSetupCodebookLookupType {
    None = 0,
    Implicitly = 1,
    Explicitly = 2,
}

impl From<u8> for VorbisSetupCodebookLookupType {
    fn from(value: u8) -> Self {
        match value {
            0 => VorbisSetupCodebookLookupType::None,
            1 => VorbisSetupCodebookLookupType::Implicitly,
            2 => VorbisSetupCodebookLookupType::Explicitly,
            _ => panic!("Invalid value for VorbisSetupCodebookLookupType: {}", value),
        }
    }
}

#[derive(Debug)]
pub enum IVorbisFloor {
    Type0(IVorbisFloorType0),
    Type1(IVorbisFloorType1),
}

/// Represents a Vorbis floor type 0.
#[derive(Debug)]
pub struct IVorbisFloorType0 {
    pub order: u8,
    pub rate: u16,
    pub bark_map_size: u16,
    pub amplitude_bits: u8,
    pub amplitude_offset: u8,
    pub number_of_books: u8,
    pub book_list: Vec<u8>,
}

/// Represents a Vorbis floor type 1.
#[derive(Debug)]
pub struct IVorbisFloorType1 {
    pub partitions: u8,
    pub partition_class_list: Vec<u8>,
    pub class_dimensions: Vec<u8>,
    pub class_subclasses: Vec<u8>,
    pub class_masterbooks: Vec<i32>,
    pub subclass_books: Vec<Vec<i32>>,
    pub multiplier: u8,
    pub rangebits: u8,
    pub x_list: Vec<i32>,
    pub values: u8,
}

/// Represents a Vorbis residue.
#[derive(Debug)]
pub struct IVorbisResidue {
    pub residue_type: u16,
    pub begin: u32,
    pub end: u32,
    pub partition_size: u32,
    pub classifications: u8,
    pub classbook: u8,
    pub cascade: Vec<u8>,
    pub books: Vec<Vec<i32>>,
}

/// Represents a Vorbis mapping.
#[derive(Debug)]
pub struct IVorbisMapping {
    pub submaps: u8,
    pub coupling_steps: u8,
    pub magnitude: Vec<u8>,
    pub angle: Vec<u8>,
    pub mux: Vec<u8>,
    pub submap_floors: Vec<u8>,
    pub submap_residues: Vec<u8>,
}

/// Represents a Vorbis mode.
#[derive(Debug)]
pub struct IVorbisMode {
    pub blockflag: bool,
    pub windowtype: u16,
    pub transformtype: u16,
    pub mapping: u8,
}

impl Deref for OggVorbisPage {
    type Target = OggPage;

    fn deref(&self) -> &Self::Target {
        &self.ogg_page
    }
}

impl DerefMut for OggVorbisPage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ogg_page
    }
}
