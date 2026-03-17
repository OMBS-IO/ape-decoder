use std::io::{Read, Seek, SeekFrom};

use crate::error::{ApeError, ApeResult};

// ---------------------------------------------------------------------------
// APE tag flag constants
// ---------------------------------------------------------------------------

pub const APE_TAG_FLAG_CONTAINS_HEADER: u32 = 1 << 31;
pub const APE_TAG_FLAG_CONTAINS_FOOTER: u32 = 1 << 30;
pub const APE_TAG_FLAG_IS_HEADER: u32 = 1 << 29;

pub const TAG_FIELD_FLAG_READ_ONLY: u32 = 1 << 0;
pub const TAG_FIELD_FLAG_DATA_TYPE_MASK: u32 = 0x06;
pub const TAG_FIELD_FLAG_DATA_TYPE_TEXT_UTF8: u32 = 0 << 1;
pub const TAG_FIELD_FLAG_DATA_TYPE_BINARY: u32 = 1 << 1;
pub const TAG_FIELD_FLAG_DATA_TYPE_EXTERNAL_INFO: u32 = 2 << 1;
pub const TAG_FIELD_FLAG_DATA_TYPE_RESERVED: u32 = 3 << 1;

const APE_TAG_FOOTER_BYTES: u32 = 32;
const APE_TAG_MAGIC: &[u8; 8] = b"APETAGEX";
const ID3V1_TAG_BYTES: u64 = 128;
const MAX_FIELD_DATA_BYTES: u32 = 256 * 1024 * 1024;
const MAX_TAG_FIELDS: u32 = 65536;
const MAX_TAG_VERSION: u32 = 2000;

// ---------------------------------------------------------------------------
// Standard APE tag field names
// ---------------------------------------------------------------------------

pub mod field_names {
    pub const TITLE: &str = "Title";
    pub const ARTIST: &str = "Artist";
    pub const ALBUM: &str = "Album";
    pub const ALBUM_ARTIST: &str = "Album Artist";
    pub const COMMENT: &str = "Comment";
    pub const YEAR: &str = "Year";
    pub const TRACK: &str = "Track";
    pub const DISC: &str = "Disc";
    pub const GENRE: &str = "Genre";
    pub const COVER_ART_FRONT: &str = "Cover Art (front)";
    pub const NOTES: &str = "Notes";
    pub const LYRICS: &str = "Lyrics";
    pub const COPYRIGHT: &str = "Copyright";
    pub const BUY_URL: &str = "Buy URL";
    pub const ARTIST_URL: &str = "Artist URL";
    pub const PUBLISHER_URL: &str = "Publisher URL";
    pub const FILE_URL: &str = "File URL";
    pub const COPYRIGHT_URL: &str = "Copyright URL";
    pub const TOOL_NAME: &str = "Tool Name";
    pub const TOOL_VERSION: &str = "Tool Version";
    pub const PEAK_LEVEL: &str = "Peak Level";
    pub const REPLAY_GAIN_RADIO: &str = "Replay Gain (radio)";
    pub const REPLAY_GAIN_ALBUM: &str = "Replay Gain (album)";
    pub const COMPOSER: &str = "Composer";
    pub const CONDUCTOR: &str = "Conductor";
    pub const ORCHESTRA: &str = "Orchestra";
    pub const KEYWORDS: &str = "Keywords";
    pub const RATING: &str = "Rating";
    pub const PUBLISHER: &str = "Publisher";
    pub const BPM: &str = "BPM";
}

// ---------------------------------------------------------------------------
// TagFieldType enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagFieldType {
    TextUtf8,
    Binary,
    ExternalInfo,
    Reserved,
}

// ---------------------------------------------------------------------------
// ApeTagField
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ApeTagField {
    pub name: String,
    pub value: Vec<u8>,
    pub flags: u32,
}

impl ApeTagField {
    /// Returns the data type of this field based on its flags.
    pub fn field_type(&self) -> TagFieldType {
        match self.flags & TAG_FIELD_FLAG_DATA_TYPE_MASK {
            TAG_FIELD_FLAG_DATA_TYPE_TEXT_UTF8 => TagFieldType::TextUtf8,
            TAG_FIELD_FLAG_DATA_TYPE_BINARY => TagFieldType::Binary,
            TAG_FIELD_FLAG_DATA_TYPE_EXTERNAL_INFO => TagFieldType::ExternalInfo,
            _ => TagFieldType::Reserved,
        }
    }

    /// Returns true if this field is marked read-only.
    pub fn is_read_only(&self) -> bool {
        self.flags & TAG_FIELD_FLAG_READ_ONLY != 0
    }

    /// Attempts to interpret the value as a UTF-8 string.
    /// Returns `None` if the field is not a text field or the value is not valid UTF-8.
    pub fn value_as_str(&self) -> Option<&str> {
        if self.field_type() != TagFieldType::TextUtf8 {
            return None;
        }
        std::str::from_utf8(&self.value).ok()
    }
}

// ---------------------------------------------------------------------------
// ApeTag
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ApeTag {
    pub version: u32,
    pub fields: Vec<ApeTagField>,
    pub has_header: bool,
}

impl ApeTag {
    /// Case-insensitive field lookup by name.
    pub fn field(&self, name: &str) -> Option<&ApeTagField> {
        let name_lower = name.to_ascii_lowercase();
        self.fields
            .iter()
            .find(|f| f.name.to_ascii_lowercase() == name_lower)
    }

    /// Convenience method: get the string value of a text field by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.field(name).and_then(|f| f.value_as_str())
    }

    pub fn title(&self) -> Option<&str> {
        self.get(field_names::TITLE)
    }

    pub fn artist(&self) -> Option<&str> {
        self.get(field_names::ARTIST)
    }

    pub fn album(&self) -> Option<&str> {
        self.get(field_names::ALBUM)
    }

    pub fn year(&self) -> Option<&str> {
        self.get(field_names::YEAR)
    }

    pub fn track(&self) -> Option<&str> {
        self.get(field_names::TRACK)
    }

    pub fn genre(&self) -> Option<&str> {
        self.get(field_names::GENRE)
    }

    pub fn comment(&self) -> Option<&str> {
        self.get(field_names::COMMENT)
    }
}

// ---------------------------------------------------------------------------
// Tag reading
// ---------------------------------------------------------------------------

/// Reads an APE tag from the end of a seekable stream.
///
/// Returns `Ok(None)` if no APE tag is found (file too small, no valid footer, etc.).
/// Returns `Ok(Some(tag))` on success, or `Err(...)` on I/O or format errors.
pub fn read_tag<R: Read + Seek>(reader: &mut R) -> ApeResult<Option<ApeTag>> {
    // Get file size
    let file_size = reader.seek(SeekFrom::End(0))?;

    // Need at least 32 bytes for a footer
    if file_size < APE_TAG_FOOTER_BYTES as u64 {
        return Ok(None);
    }

    // Check for ID3v1 tag at end of file
    let has_id3v1 = if file_size >= ID3V1_TAG_BYTES {
        reader.seek(SeekFrom::End(-(ID3V1_TAG_BYTES as i64)))?;
        let mut id3_header = [0u8; 3];
        reader.read_exact(&mut id3_header)?;
        &id3_header == b"TAG"
    } else {
        false
    };

    // Determine where the APE tag footer should be
    let footer_end = if has_id3v1 {
        file_size - ID3V1_TAG_BYTES
    } else {
        file_size
    };

    if footer_end < APE_TAG_FOOTER_BYTES as u64 {
        return Ok(None);
    }

    let footer_start = footer_end - APE_TAG_FOOTER_BYTES as u64;

    // Read the 32-byte footer
    reader.seek(SeekFrom::Start(footer_start))?;
    let mut footer_buf = [0u8; 32];
    reader.read_exact(&mut footer_buf)?;

    // Validate magic
    if &footer_buf[0..8] != APE_TAG_MAGIC {
        return Ok(None);
    }

    // Parse footer fields
    let version = u32::from_le_bytes(footer_buf[8..12].try_into().unwrap());
    let size = u32::from_le_bytes(footer_buf[12..16].try_into().unwrap());
    let num_fields = u32::from_le_bytes(footer_buf[16..20].try_into().unwrap());
    let flags = u32::from_le_bytes(footer_buf[20..24].try_into().unwrap());

    // The footer itself must not be a header
    if flags & APE_TAG_FLAG_IS_HEADER != 0 {
        return Ok(None);
    }

    // Validate footer
    if version > MAX_TAG_VERSION {
        return Err(ApeError::InvalidFormat("APE tag version too high"));
    }
    if num_fields > MAX_TAG_FIELDS {
        return Err(ApeError::InvalidFormat("APE tag has too many fields"));
    }
    if size < APE_TAG_FOOTER_BYTES {
        return Err(ApeError::InvalidFormat("APE tag size too small"));
    }
    let field_bytes = size - APE_TAG_FOOTER_BYTES;
    if field_bytes > MAX_FIELD_DATA_BYTES {
        return Err(ApeError::InvalidFormat("APE tag field data too large"));
    }

    let has_header = flags & APE_TAG_FLAG_CONTAINS_HEADER != 0;

    // The tag's size field includes the footer but not the header.
    // Field data starts at footer_end - size.
    let field_data_start = footer_end
        .checked_sub(size as u64)
        .ok_or(ApeError::InvalidFormat(
            "APE tag size extends before start of file",
        ))?;

    if field_bytes == 0 {
        return Ok(Some(ApeTag {
            version,
            fields: Vec::new(),
            has_header,
        }));
    }

    // Read field data
    reader.seek(SeekFrom::Start(field_data_start))?;
    let mut field_data = vec![0u8; field_bytes as usize];
    reader.read_exact(&mut field_data)?;

    // Parse fields
    let fields = parse_fields(&field_data, num_fields)?;

    Ok(Some(ApeTag {
        version,
        fields,
        has_header,
    }))
}

/// Parse tag fields from raw field data bytes.
fn parse_fields(data: &[u8], num_fields: u32) -> ApeResult<Vec<ApeTagField>> {
    let mut fields = Vec::with_capacity(num_fields as usize);
    let mut offset = 0usize;

    for _ in 0..num_fields {
        // Need at least 8 bytes for value_size + flags
        if offset + 8 > data.len() {
            return Err(ApeError::InvalidFormat("APE tag field truncated (header)"));
        }

        let value_size = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        let field_flags = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());
        offset += 8;

        // Find null terminator for the field name
        let name_start = offset;
        let name_end = data[name_start..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| name_start + pos)
            .ok_or(ApeError::InvalidFormat(
                "APE tag field name not null-terminated",
            ))?;

        // Validate field name is printable ASCII (0x20..=0x7E)
        let name_bytes = &data[name_start..name_end];
        if name_bytes.is_empty() {
            return Err(ApeError::InvalidFormat("APE tag field name is empty"));
        }
        for &b in name_bytes {
            if b < 0x20 || b > 0x7E {
                return Err(ApeError::InvalidFormat(
                    "APE tag field name contains non-printable character",
                ));
            }
        }
        let name = String::from_utf8(name_bytes.to_vec())
            .map_err(|_| ApeError::InvalidFormat("APE tag field name is not valid ASCII"))?;

        // Skip past null terminator
        offset = name_end + 1;

        // Read value
        let value_size = value_size as usize;
        if offset + value_size > data.len() {
            return Err(ApeError::InvalidFormat("APE tag field value truncated"));
        }
        let value = data[offset..offset + value_size].to_vec();
        offset += value_size;

        fields.push(ApeTagField {
            name,
            value,
            flags: field_flags,
        });
    }

    Ok(fields)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper: build a synthetic APE tag byte stream with the given fields.
    /// Each field is (name, value, flags). Returns the complete byte buffer
    /// (field data + footer), optionally followed by an ID3v1 block.
    fn build_tag(fields: &[(&str, &[u8], u32)], with_id3v1: bool, with_header: bool) -> Vec<u8> {
        let mut field_data = Vec::new();
        for &(name, value, flags) in fields {
            field_data.extend_from_slice(&(value.len() as u32).to_le_bytes());
            field_data.extend_from_slice(&flags.to_le_bytes());
            field_data.extend_from_slice(name.as_bytes());
            field_data.push(0); // null terminator
            field_data.extend_from_slice(value);
        }

        let field_bytes = field_data.len() as u32;
        let tag_size = field_bytes + APE_TAG_FOOTER_BYTES;

        let mut tag_flags = APE_TAG_FLAG_CONTAINS_FOOTER;
        if with_header {
            tag_flags |= APE_TAG_FLAG_CONTAINS_HEADER;
        }

        // Build footer
        let mut footer = Vec::new();
        footer.extend_from_slice(APE_TAG_MAGIC);
        footer.extend_from_slice(&2000u32.to_le_bytes());
        footer.extend_from_slice(&tag_size.to_le_bytes());
        footer.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        footer.extend_from_slice(&tag_flags.to_le_bytes());
        footer.extend_from_slice(&[0u8; 8]);

        let mut buf = Vec::new();

        // Optional header (same structure as footer but with IS_HEADER flag)
        if with_header {
            let mut header = Vec::new();
            header.extend_from_slice(APE_TAG_MAGIC);
            header.extend_from_slice(&2000u32.to_le_bytes());
            header.extend_from_slice(&tag_size.to_le_bytes());
            header.extend_from_slice(&(fields.len() as u32).to_le_bytes());
            header.extend_from_slice(&(tag_flags | APE_TAG_FLAG_IS_HEADER).to_le_bytes());
            header.extend_from_slice(&[0u8; 8]);
            buf.extend_from_slice(&header);
        }

        buf.extend_from_slice(&field_data);
        buf.extend_from_slice(&footer);

        if with_id3v1 {
            let mut id3v1 = vec![0u8; 128];
            id3v1[0] = b'T';
            id3v1[1] = b'A';
            id3v1[2] = b'G';
            buf.extend_from_slice(&id3v1);
        }

        buf
    }

    /// Build a realistic tag that mimics what the MAC tool writes.
    fn build_mac_tool_tag() -> Vec<u8> {
        build_tag(
            &[
                (field_names::TOOL_NAME, b"Monkey's Audio", 0),
                (field_names::TOOL_VERSION, b"10.44", 0),
                (field_names::TITLE, b"Sine Wave", 0),
                (field_names::ARTIST, b"Test Generator", 0),
                (field_names::ALBUM, b"Test Signals", 0),
                (field_names::YEAR, b"2024", 0),
                (field_names::TRACK, b"1", 0),
                (field_names::GENRE, b"Test", 0),
                (field_names::COMMENT, b"Generated for testing", 0),
            ],
            false,
            false,
        )
    }

    #[test]
    fn read_tag_from_fixture() {
        // The test fixture files were generated without APE tags.
        // Verify that read_tag gracefully returns None.
        let mut file = std::fs::File::open(
            "/home/johns/repos/ape/decoder/tests/fixtures/ape/sine_16s_c2000.ape",
        )
        .expect("failed to open test fixture");
        let result = read_tag(&mut file).expect("read_tag should not error");
        assert!(result.is_none(), "fixture has no APE tag");
    }

    #[test]
    fn tool_name_and_version_fields_exist() {
        let data = build_mac_tool_tag();
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        let tool_name = tag.get(field_names::TOOL_NAME);
        assert_eq!(tool_name, Some("Monkey's Audio"));

        let tool_version = tag.get(field_names::TOOL_VERSION);
        assert_eq!(tool_version, Some("10.44"));
    }

    #[test]
    fn case_insensitive_field_lookup() {
        let data = build_mac_tool_tag();
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        // Look up "Tool Name" with different casing
        let upper = tag.get("TOOL NAME");
        let lower = tag.get("tool name");
        let mixed = tag.get("Tool Name");

        assert_eq!(upper, mixed);
        assert_eq!(lower, mixed);
        assert_eq!(mixed, Some("Monkey's Audio"));

        // Standard accessors
        assert_eq!(tag.title(), Some("Sine Wave"));
        assert_eq!(tag.artist(), Some("Test Generator"));
        assert_eq!(tag.album(), Some("Test Signals"));
        assert_eq!(tag.year(), Some("2024"));
        assert_eq!(tag.track(), Some("1"));
        assert_eq!(tag.genre(), Some("Test"));
        assert_eq!(tag.comment(), Some("Generated for testing"));
    }

    #[test]
    fn nonexistent_field_returns_none() {
        let data = build_mac_tool_tag();
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        assert!(tag.field("Nonexistent Field 12345").is_none());
        assert!(tag.get("Nonexistent Field 12345").is_none());
    }

    #[test]
    fn value_as_str_for_text_fields() {
        let data = build_mac_tool_tag();
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        for field in &tag.fields {
            if field.field_type() == TagFieldType::TextUtf8 {
                assert!(
                    field.value_as_str().is_some(),
                    "text field '{}' should be valid UTF-8",
                    field.name
                );
            }
        }
    }

    #[test]
    fn value_as_str_returns_none_for_binary() {
        let field = ApeTagField {
            name: "test".to_string(),
            value: vec![0xFF, 0xFE],
            flags: TAG_FIELD_FLAG_DATA_TYPE_BINARY,
        };
        assert!(field.value_as_str().is_none());
    }

    #[test]
    fn field_type_classification() {
        let text = ApeTagField {
            name: "t".into(),
            value: vec![],
            flags: TAG_FIELD_FLAG_DATA_TYPE_TEXT_UTF8,
        };
        assert_eq!(text.field_type(), TagFieldType::TextUtf8);

        let binary = ApeTagField {
            name: "b".into(),
            value: vec![],
            flags: TAG_FIELD_FLAG_DATA_TYPE_BINARY,
        };
        assert_eq!(binary.field_type(), TagFieldType::Binary);

        let external = ApeTagField {
            name: "e".into(),
            value: vec![],
            flags: TAG_FIELD_FLAG_DATA_TYPE_EXTERNAL_INFO,
        };
        assert_eq!(external.field_type(), TagFieldType::ExternalInfo);

        let reserved = ApeTagField {
            name: "r".into(),
            value: vec![],
            flags: TAG_FIELD_FLAG_DATA_TYPE_RESERVED,
        };
        assert_eq!(reserved.field_type(), TagFieldType::Reserved);
    }

    #[test]
    fn is_read_only_flag() {
        let ro = ApeTagField {
            name: "ro".into(),
            value: vec![],
            flags: TAG_FIELD_FLAG_READ_ONLY,
        };
        assert!(ro.is_read_only());

        let rw = ApeTagField {
            name: "rw".into(),
            value: vec![],
            flags: 0,
        };
        assert!(!rw.is_read_only());
    }

    #[test]
    fn file_too_small_returns_none() {
        let data = vec![0u8; 16];
        let mut cursor = Cursor::new(data);
        let result = read_tag(&mut cursor).expect("should not error");
        assert!(result.is_none());
    }

    #[test]
    fn no_tag_returns_none() {
        // 64 bytes of zeros -- no APETAGEX magic
        let data = vec![0u8; 64];
        let mut cursor = Cursor::new(data);
        let result = read_tag(&mut cursor).expect("should not error");
        assert!(result.is_none());
    }

    #[test]
    fn synthetic_minimal_tag() {
        let data = build_tag(&[("Test", b"Hello", 0)], false, false);
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        assert_eq!(tag.version, 2000);
        assert_eq!(tag.fields.len(), 1);
        assert_eq!(tag.get("Test"), Some("Hello"));
        assert_eq!(tag.get("test"), Some("Hello")); // case-insensitive
        assert!(!tag.has_header);
    }

    #[test]
    fn synthetic_tag_with_id3v1() {
        let data = build_tag(&[("Foo", b"Bar", 0)], true, false);
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        assert_eq!(tag.get("Foo"), Some("Bar"));
    }

    #[test]
    fn synthetic_tag_with_header() {
        let data = build_tag(&[("Artist", b"Someone", 0)], false, true);
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        assert!(tag.has_header);
        assert_eq!(tag.artist(), Some("Someone"));
    }

    #[test]
    fn multiple_fields_parsed_correctly() {
        let data = build_mac_tool_tag();
        let mut cursor = Cursor::new(data);
        let tag = read_tag(&mut cursor)
            .expect("read_tag failed")
            .expect("expected tag");

        assert_eq!(tag.version, 2000);
        assert_eq!(tag.fields.len(), 9);
    }
}
