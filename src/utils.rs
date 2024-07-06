use crate::ogg_vorbis_page::{IVorbisCommentHeader, OggVorbisPage};
use crate::read_ogg_vorbis_file::{read_ogg_vorbis_file, OggVorbisPacket, OggVorbisPageResult};
use std::io::{self, Error, ErrorKind};
use tokio::io::AsyncReadExt;

/// Asynchronously trims an Ogg Vorbis file by removing segments before the first header and data after the last segment.
///
/// This function reads the Ogg Vorbis file, searches for the first header (Identification, Comment, or Setup),
/// and removes all segments before this header. The trimmed file is returned as a vector of `OggVorbisPageResult`.
///
/// # Arguments
///
/// * `reader` - An asynchronous reader implementing `AsyncReadExt` and `Unpin`.
/// * `tolerate` - A boolean indicating whether to tolerate minor errors during reading.
/// * `header_search_range` - The range in bytes to search for the header.
///
/// # Returns
///
/// A result containing a vector of `OggVorbisPageResult` if successful, or an `io::Error` if an error occurs.
pub async fn trim_ogg_vorbis_file<R: AsyncReadExt + Unpin>(
    mut reader: R,
    tolerate: bool,
    header_search_range: usize,
) -> io::Result<Vec<OggVorbisPageResult>> {
    let mut result = Vec::new();
    let mut header_found = false;

    let page_results = read_ogg_vorbis_file(&mut reader, tolerate, header_search_range).await?;
    for mut page_result in page_results {
        if !header_found {
            if let Some(found_header_index) = page_result.packets.iter().position(|packet| {
                matches!(
                    packet,
                    OggVorbisPacket::Identification(_)
                        | OggVorbisPacket::Comment(_)
                        | OggVorbisPacket::Setup(_)
                )
            }) {
                header_found = true;
                if found_header_index > 0 {
                    page_result.page = page_result.page
                        .remove_page_segment(0, found_header_index)
                        .map_err(|e| Error::new(ErrorKind::Other, e))?;
                    page_result.packets = page_result.packets.split_off(found_header_index);
                    page_result.page.update_page_checksum();
                }
            } else {
                continue;
            }
        }
        result.push(page_result);
    }

    Ok(result)
}

/// Asynchronously collects all pages of an Ogg Vorbis file.
///
/// This function reads the entire Ogg Vorbis file and returns a vector of `OggVorbisPageResult` containing
/// all the pages found in the file.
///
/// # Arguments
///
/// * `reader` - An asynchronous reader implementing `AsyncReadExt` and `Unpin`.
/// * `tolerate` - A boolean indicating whether to tolerate minor errors during reading.
/// * `header_search_range` - The range in bytes to search for the header.
///
/// # Returns
///
/// A result containing a vector of `OggVorbisPageResult` if successful, or an `io::Error` if an error occurs.
pub async fn collect_ogg_vorbis_file<R: AsyncReadExt + Unpin>(
    mut reader: R,
    tolerate: bool,
    header_search_range: usize,
) -> io::Result<Vec<OggVorbisPageResult>> {
    let mut result = Vec::new();

    let page_results = read_ogg_vorbis_file(&mut reader, tolerate, header_search_range).await?;
    for page_result in page_results {
        result.push(page_result);
    }

    Ok(result)
}

/// Finds a packet of a specified type in an Ogg Vorbis file.
///
/// This function searches through the provided `ogg_vorbis_file` for a packet of the specified type
/// ("identification", "comment", or "setup") and returns its position as a tuple of page and packet indices.
///
/// # Arguments
///
/// * `ogg_vorbis_file` - A slice of `OggVorbisPageResult` representing the Ogg Vorbis file.
/// * `packet_type` - A string specifying the type of packet to find ("identification", "comment", or "setup").
///
/// # Returns
///
/// An option containing a tuple of page and packet indices if the packet is found, or `None` if not found.
pub fn find_packet_by_type(
    ogg_vorbis_file: &[OggVorbisPageResult],
    packet_type: &str,
) -> Option<(usize, usize)> {
    ogg_vorbis_file
        .iter()
        .enumerate()
        .find_map(|(page_index, page)| {
            page.packets
                .iter()
                .enumerate()
                .find_map(|(packet_index, packet)| match packet {
                    OggVorbisPacket::Identification(_) if packet_type == "identification" => {
                        Some((page_index, packet_index))
                    }
                    OggVorbisPacket::Comment(_) if packet_type == "comment" => {
                        Some((page_index, packet_index))
                    }
                    OggVorbisPacket::Setup(_) if packet_type == "setup" => {
                        Some((page_index, packet_index))
                    }
                    _ => None,
                })
        })
}

/// Updates the comments in an Ogg Vorbis file.
///
/// This function replaces the comment packet at the specified position with new comments provided
/// as a `HashMap`. The updated Ogg Vorbis file is returned as a vector of `OggVorbisPageResult`.
///
/// # Arguments
///
/// * `ogg_vorbis_file` - A vector of `OggVorbisPageResult` representing the Ogg Vorbis file.
/// * `comments_page_index` - The index of the page containing the comment packet to update.
/// * `comments_index` - The index of the comment packet within the specified page.
/// * `new_comments` - A `HashMap` containing the new comments to replace the existing ones.
///
/// # Returns
///
/// A vector of `OggVorbisPageResult` representing the updated Ogg Vorbis file.
pub fn update_ogg_vorbis_comments(
    mut ogg_vorbis_file: Vec<OggVorbisPageResult>,
    comments_page_index: usize,
    comments_index: usize,
    new_comments: std::collections::HashMap<String, Vec<String>>,
) -> Vec<OggVorbisPageResult> {
    if comments_page_index != usize::MAX {
        let comments_page = &mut ogg_vorbis_file[comments_page_index];
        if let OggVorbisPacket::Comment(ref mut comment_packet) =
            comments_page.packets[comments_index]
        {
            let new_comment_header = IVorbisCommentHeader {
                vendor: comment_packet.data.vendor.clone(),
                comments: new_comments,
            };
            let new_comment_segment = OggVorbisPage::build_comments(new_comment_header);
            comments_page.page = comments_page
                .page
                .replace_page_segment(new_comment_segment, comments_index)
                .expect("Failed to replace page segment");
        }
    }

    ogg_vorbis_file
}
