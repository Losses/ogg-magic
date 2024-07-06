pub mod ogg_page;
pub use ogg_page::{OggFormatError, OggPage};

pub mod crc;
pub use crc::vorbis_crc32;

pub mod ogg_vorbis_page;
pub use ogg_vorbis_page::{
    float32_unpack, ilog, BitStreamReader, IVorbisCommentHeader, IVorbisFloor, IVorbisFloorType0,
    IVorbisFloorType1, IVorbisIdentificationHeader, IVorbisMapping, IVorbisMode, IVorbisResidue,
    IVorbisSetupCodebook, IVorbisSetupHeader, OggVorbisPage, VorbisHeaderType,
    VorbisSetupCodebookLookupType,
};

pub mod read_ogg_vorbis_file;
pub use read_ogg_vorbis_file::{
    read_ogg_vorbis_file, OggParseResult, OggVorbisPacketsParseResult, OggVorbisPageResult,
    OggVorbiseHeaderCommentParseResult, OggVorbiseHeaderIdentificationParseResult,
    OggVorbiseHeaderSetupParseResult,
};

pub mod utils;
pub use utils::{
    collect_ogg_vorbis_file, find_packet_by_type, trim_ogg_vorbis_file, update_ogg_vorbis_comments,
};
