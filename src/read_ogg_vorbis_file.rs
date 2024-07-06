use std::io::{self};
use tokio::io::AsyncReadExt;

use crate::ogg_vorbis_page::{
    IVorbisCommentHeader, IVorbisIdentificationHeader, IVorbisSetupHeader, OggVorbisPage,
};

/// Represents the result of parsing an Ogg page, containing the type of result,
/// the parsed data, the index of the segment, and the raw segment data.
#[derive(Debug)]
pub struct OggParseResult<T> {
    /// The type of result (e.g., "identification", "comment", "setup", "body").
    pub result_type: String,
    /// The parsed data of type `T`.
    pub data: T,
    /// The index of the segment within the page.
    pub index: usize,
    /// The raw segment data.
    pub get_raw_segment: Vec<u8>,
}

/// Type alias for the result of parsing an Ogg Vorbis identification header.
pub type OggVorbiseHeaderIdentificationParseResult = OggParseResult<IVorbisIdentificationHeader>;
/// Type alias for the result of parsing an Ogg Vorbis comment header.
pub type OggVorbiseHeaderCommentParseResult = OggParseResult<IVorbisCommentHeader>;
/// Type alias for the result of parsing an Ogg Vorbis setup header.
pub type OggVorbiseHeaderSetupParseResult = OggParseResult<IVorbisSetupHeader>;
/// Type alias for the result of parsing Ogg Vorbis packets.
pub type OggVorbisPacketsParseResult = OggParseResult<()>;

/// Enum representing different types of Ogg Vorbis packets.
#[derive(Debug)]
pub enum OggVorbisPacket {
    /// Identification packet.
    Identification(OggVorbiseHeaderIdentificationParseResult),
    /// Comment packet.
    Comment(OggVorbiseHeaderCommentParseResult),
    /// Setup packet.
    Setup(OggVorbiseHeaderSetupParseResult),
    /// Body packet.
    Body(OggVorbisPacketsParseResult),
}

/// Represents the result of parsing an Ogg Vorbis page, containing the page
/// and its associated packets.
#[derive(Debug)]
pub struct OggVorbisPageResult {
    /// The parsed Ogg Vorbis page.
    pub page: OggVorbisPage,
    /// The packets contained within the page.
    pub packets: Vec<OggVorbisPacket>,
}

/// Reads an Ogg Vorbis file asynchronously and parses its pages and packets.
///
/// # Arguments
///
/// * `reader` - A mutable reference to an asynchronous reader implementing `AsyncReadExt`.
/// * `tolerate` - A boolean indicating whether to tolerate errors and continue parsing.
/// * `header_search_range` - The range within which to search for headers.
///
/// # Returns
///
/// A `Result` containing a vector of `OggVorbisPageResult` on success, or an `io::Error` on failure.
pub async fn read_ogg_vorbis_file<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    tolerate: bool,
    header_search_range: usize,
) -> io::Result<Vec<OggVorbisPageResult>> {
    let mut done = false;
    let mut buffer: Vec<u8> = Vec::new();
    let mut audio_channels: Option<u8> = None;
    let mut results = Vec::new();

    while !done || !buffer.is_empty() {
        let mut chunk = vec![0; 4096];
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            done = true;
        } else {
            buffer.extend_from_slice(&chunk[..n]);
        }

        if buffer.len() < 4 && done {
            break;
        }

        // Try to parse the Ogg Vorbis page
        let page = match OggVorbisPage::new(buffer.clone()) {
            Ok(page) => page,
            Err(error) => {
                if done {
                    if tolerate {
                        // Move forward one byte and continue
                        buffer.drain(..1);
                        continue;
                    } else {
                        // If done and still error, return the error
                        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
                    }
                } else {
                    // If not done, wait for more data
                    continue;
                }
            }
        };

        let mut result = OggVorbisPageResult {
            page: page.clone(),
            packets: Vec::new(),
        };

        for (accumulated_segments, segment) in (0..page.ogg_page.page_segments).enumerate() {
            if accumulated_segments > header_search_range {
                result.packets.push(OggVorbisPacket::Body(OggParseResult {
                    result_type: String::from("body"),
                    data: (),
                    index: segment,
                    get_raw_segment: page.ogg_page.get_page_segment(segment).unwrap(),
                }));
            } else if page.is_identification_packet(segment) {
                let identification = page.get_identification(segment).unwrap();
                audio_channels = Some(identification.audio_channels);

                result.packets.push(OggVorbisPacket::Identification(
                    OggVorbiseHeaderIdentificationParseResult {
                        result_type: String::from("identification"),
                        data: identification,
                        index: segment,
                        get_raw_segment: page.ogg_page.get_page_segment(segment).unwrap(),
                    },
                ));
            } else if page.is_comment_packet(segment) {
                let comments = page.get_comments(segment).unwrap();

                result.packets.push(OggVorbisPacket::Comment(
                    OggVorbiseHeaderCommentParseResult {
                        result_type: String::from("comment"),
                        data: comments,
                        index: segment,
                        get_raw_segment: page.ogg_page.get_page_segment(segment).unwrap(),
                    },
                ));
            } else if page.is_setup_packet(segment) {
                if let Some(channels) = audio_channels {
                    let setup = page.get_setup(channels, segment).unwrap();

                    result
                        .packets
                        .push(OggVorbisPacket::Setup(OggVorbiseHeaderSetupParseResult {
                            result_type: String::from("setup"),
                            data: setup,
                            index: segment,
                            get_raw_segment: page.ogg_page.get_page_segment(segment).unwrap(),
                        }));
                } else {
                    println!(
                        "Found a setup packet but no identification packet detected, will skip"
                    );
                }
            } else {
                result.packets.push(OggVorbisPacket::Body(OggParseResult {
                    result_type: String::from("body"),
                    data: (),
                    index: segment,
                    get_raw_segment: page.ogg_page.get_page_segment(segment).unwrap(),
                }));
            }
        }

        // Update buffer to remove the processed page
        buffer.drain(..page.ogg_page.page_size);
        results.push(result);
    }

    Ok(results)
}
