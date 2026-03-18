// Allow clippy style lints that conflict with the codec's algorithmic patterns
#![allow(
    clippy::too_many_arguments,     // decode functions mirror C++ parameter lists
    clippy::unnecessary_cast,       // explicit casts document integer width transitions
    clippy::manual_range_contains,  // boundary checks match C++ source for clarity
    clippy::new_without_default,    // codec types require explicit initialization parameters
    clippy::manual_memcpy,          // explicit loops match C++ EXPAND_N_TIMES patterns
    clippy::needless_range_loop,    // index loops needed for parallel array access
    clippy::manual_is_multiple_of,  // matches C++ modulo patterns
    clippy::implicit_saturating_sub,// explicit arithmetic matches C++ overflow semantics
    clippy::needless_lifetimes,     // explicit lifetimes clarify borrow relationships
    clippy::empty_line_after_doc_comments,
    clippy::manual_div_ceil,        // explicit div_ceil matches C++ patterns
    clippy::type_complexity          // complex closure types in thread spawning
)]

pub mod bitreader;
pub mod crc;
pub mod decoder;
pub mod entropy;
pub mod error;
pub mod format;
pub mod id3v2;
pub mod nn_filter;
pub mod predictor;
pub mod range_coder;
pub mod roll_buffer;
pub mod tag;
pub mod unprepare;

pub use decoder::{decode, ApeDecoder, ApeInfo, FrameIterator, SeekResult, SourceFormat};
pub use error::{ApeError, ApeResult};
pub use id3v2::{read_id3v2, Id3v2Frame, Id3v2Tag};
pub use tag::{read_tag, remove_tag, write_tag, ApeTag, ApeTagField, TagFieldType};
