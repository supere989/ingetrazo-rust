// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded physical collection identity lookup; outline programs are never read.
use std::io::{Read, Seek, SeekFrom};
fn read_at(file: &mut (impl Read + Seek), offset: u64, n: usize) -> Result<Vec<u8>, String> {
    if n > 256 * 1024 {
        return Err("Native identity table budget exceeded".into());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| "Native identity allocation failed")?;
    out.resize(n, 0);
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    file.read_exact(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
fn u16_at(b: &[u8], n: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(
        b.get(n..n + 2)
            .ok_or("Invalid SFNT identity")?
            .try_into()
            .unwrap(),
    ))
}
fn u32_at(b: &[u8], n: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        b.get(n..n + 4)
            .ok_or("Invalid SFNT identity")?
            .try_into()
            .unwrap(),
    ))
}
pub(crate) fn collection_index(file: &mut (impl Read + Seek), name: &str) -> Result<u32, String> {
    collection_index_match(file, name)?
        .ok_or_else(|| format!("Native PostScript identity {name:?} not found in collection"))
}
/// None is only a valid, fully inspected collection with no matching name.
/// Read/format/allocation/admission errors remain Err and cannot trigger fallback.
pub(crate) fn collection_index_match(
    file: &mut (impl Read + Seek),
    name: &str,
) -> Result<Option<u32>, String> {
    let header = read_at(file, 0, 12)?;
    let offsets = if &header[..4] == b"ttcf" {
        let count = u32_at(&header, 8)?;
        if count == 0 || count > 4096 {
            return Err("Native collection face budget exceeded".into());
        }
        let raw = read_at(file, 12, count as usize * 4)?;
        (0..count as usize)
            .map(|i| u32_at(&raw, i * 4))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![0]
    };
    for (index, offset) in offsets.into_iter().enumerate() {
        let h = read_at(file, u64::from(offset), 12)?;
        let count = u16_at(&h, 4)?;
        if count > 256 {
            return Err("Native SFNT directory budget exceeded".into());
        }
        let directory = read_at(file, u64::from(offset) + 12, count as usize * 16)?;
        for row in directory.chunks_exact(16) {
            if &row[..4] != b"name" {
                continue;
            }
            let table = read_at(file, u64::from(u32_at(row, 8)?), u32_at(row, 12)? as usize)?;
            let count = u16_at(&table, 2)? as usize;
            let strings = u16_at(&table, 4)? as usize;
            for i in 0..count {
                let r = 6 + i * 12;
                let platform = u16_at(&table, r)?;
                let encoding = u16_at(&table, r + 2)?;
                if u16_at(&table, r + 6)? != 6 {
                    continue;
                }
                let n = u16_at(&table, r + 8)? as usize;
                let start = strings
                    .checked_add(u16_at(&table, r + 10)? as usize)
                    .ok_or("Native name overflow")?;
                let bytes = table
                    .get(start..start + n)
                    .ok_or("Invalid native name range")?;
                let candidate =
                    if platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10)) {
                        if bytes.len() % 2 != 0 {
                            continue;
                        }
                        String::from_utf16(
                            &bytes
                                .chunks_exact(2)
                                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                                .collect::<Vec<_>>(),
                        )
                        .ok()
                    } else if platform == 1
                        && encoding == 0
                        && bytes.iter().all(|b| (33..=126).contains(b))
                    {
                        String::from_utf8(bytes.to_vec()).ok()
                    } else {
                        None
                    };
                if candidate.as_deref() == Some(name) {
                    return Ok(Some(index as u32));
                }
            }
        }
    }
    Ok(None)
}

/// Named variable instances can have a different PostScript name from their physical
/// collection face. Match exact native metadata tables, never geometry or family names.
pub(crate) fn collection_index_by_tables(
    file: &mut (impl Read + Seek),
    head: &[u8],
    name: &[u8],
) -> Result<u32, String> {
    if head.len() != 54 || name.len() > 256 * 1024 || name.len() < 6 {
        return Err("Invalid native physical identity tables".into());
    }
    let header = read_at(file, 0, 12)?;
    let offsets = if &header[..4] == b"ttcf" {
        let count = u32_at(&header, 8)?;
        if count == 0 || count > 4096 {
            return Err("Native collection face budget exceeded".into());
        }
        let raw = read_at(file, 12, count as usize * 4)?;
        (0..count as usize)
            .map(|i| u32_at(&raw, i * 4))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![0]
    };
    let mut matched = None;
    let mut table_bytes = 0usize;
    for (index, offset) in offsets.into_iter().enumerate() {
        let header = read_at(file, u64::from(offset), 12)?;
        let count = u16_at(&header, 4)?;
        if count > 256 {
            return Err("Native SFNT directory budget exceeded".into());
        }
        let directory = read_at(file, u64::from(offset) + 12, count as usize * 16)?;
        let mut found = 0u8;
        let mut seen = 0u8;
        for row in directory.chunks_exact(16) {
            let (expected, bit) = match &row[..4] {
                b"head" => (head, 1),
                b"name" => (name, 2),
                _ => continue,
            };
            if seen & bit != 0 {
                return Err("Duplicate native physical identity table tag".into());
            }
            seen |= bit;
            if u32_at(row, 12)? as usize != expected.len() {
                continue;
            }
            table_bytes = table_bytes
                .checked_add(expected.len())
                .ok_or("Native identity byte overflow")?;
            if table_bytes > 16 * 1024 * 1024 {
                return Err("Native identity aggregate read budget exceeded".into());
            }
            let actual = read_at(file, u64::from(u32_at(row, 8)?), expected.len())?;
            if actual == expected {
                found |= bit;
            }
        }
        if found == 3 {
            if matched.is_some() {
                return Err("Ambiguous native physical collection identity".into());
            }
            matched = Some(index as u32);
        }
    }
    matched.ok_or_else(|| "Native metadata identity not found in collection".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    struct Counted {
        data: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for Counted {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = self.data.read(b)?;
            self.read += n;
            Ok(n)
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
            self.data.seek(p)
        }
    }
    fn collection() -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..4].copy_from_slice(b"ttcf");
        b[8..12].copy_from_slice(&1u32.to_be_bytes());
        b[12..16].copy_from_slice(&16u32.to_be_bytes());
        b[16..20].copy_from_slice(&0x10000u32.to_be_bytes());
        b[20..22].copy_from_slice(&2u16.to_be_bytes());
        b[28..32].copy_from_slice(b"name");
        b[36..40].copy_from_slice(&60u32.to_be_bytes());
        b[40..44].copy_from_slice(&24u32.to_be_bytes());
        b[44..48].copy_from_slice(b"hvgl");
        b[52..56].copy_from_slice(&4096u32.to_be_bytes());
        b[56..60].copy_from_slice(&53_903_096u32.to_be_bytes());
        b[62..64].copy_from_slice(&1u16.to_be_bytes());
        b[64..66].copy_from_slice(&18u16.to_be_bytes());
        b[66..68].copy_from_slice(&1u16.to_be_bytes());
        b[72..74].copy_from_slice(&6u16.to_be_bytes());
        b[74..76].copy_from_slice(&6u16.to_be_bytes());
        b[78..84].copy_from_slice(b"FacePS");
        b
    }
    #[test]
    fn duplicate_head_cannot_replace_missing_name_and_malformed_name_is_not_miss() {
        let mut b = collection();
        b.resize(200, 0);
        b[28..32].copy_from_slice(b"head");
        b[36..40].copy_from_slice(&128u32.to_be_bytes());
        b[40..44].copy_from_slice(&54u32.to_be_bytes());
        b[44..48].copy_from_slice(b"head");
        b[52..56].copy_from_slice(&128u32.to_be_bytes());
        b[56..60].copy_from_slice(&54u32.to_be_bytes());
        let head = b[128..182].to_vec();
        let name = collection()[60..84].to_vec();
        assert!(
            collection_index_by_tables(&mut Cursor::new(b), &head, &name)
                .unwrap_err()
                .contains("Duplicate")
        );
        let mut b = collection();
        b[40..44].copy_from_slice(&300_000u32.to_be_bytes());
        assert!(collection_index_match(&mut Cursor::new(b), "Absent")
            .unwrap_err()
            .contains("budget"));
        assert_eq!(
            collection_index_match(&mut Cursor::new(collection()), "Absent").unwrap(),
            None
        );
    }
    #[test]
    fn named_instance_uses_exact_metadata_not_requested_postscript() {
        let mut b = collection();
        b.resize(200, 0);
        b[44..48].copy_from_slice(b"head");
        b[52..56].copy_from_slice(&128u32.to_be_bytes());
        b[56..60].copy_from_slice(&54u32.to_be_bytes());
        let head = b[128..182].to_vec();
        let name = b[60..84].to_vec();
        assert!(collection_index(&mut Cursor::new(b.clone()), "NamedInstance").is_err());
        assert_eq!(
            collection_index_by_tables(&mut Cursor::new(b.clone()), &head, &name).unwrap(),
            0
        );
        let mut duplicate = vec![0u8; 256];
        duplicate[..12].copy_from_slice(&b[..12]);
        duplicate[8..12].copy_from_slice(&2u32.to_be_bytes());
        duplicate[12..16].copy_from_slice(&32u32.to_be_bytes());
        duplicate[16..20].copy_from_slice(&32u32.to_be_bytes());
        duplicate[32..76].copy_from_slice(&b[16..60]);
        duplicate[52..56].copy_from_slice(&100u32.to_be_bytes());
        duplicate[100..124].copy_from_slice(&name);
        duplicate[128..182].copy_from_slice(&head);
        assert!(
            collection_index_by_tables(&mut Cursor::new(duplicate), &head, &name)
                .unwrap_err()
                .contains("Ambiguous")
        );
        let mut wrong = head.clone();
        wrong[20] = 1;
        assert!(collection_index_by_tables(&mut Cursor::new(b), &wrong, &name).is_err());
    }
    #[test]
    fn physical_identity_never_reads_outline_collection_program() {
        let mut source = Counted {
            data: Cursor::new(collection()),
            read: 0,
        };
        assert_eq!(collection_index(&mut source, "FacePS").unwrap(), 0);
        assert!(source.read < 128);
        assert!(collection_index(&mut source, "OtherFace").is_err());
    }
    #[test]
    fn malformed_identity_table_refuses_before_large_allocation() {
        let mut b = collection();
        b[40..44].copy_from_slice(&300_000u32.to_be_bytes());
        assert!(collection_index(&mut Cursor::new(b), "FacePS")
            .unwrap_err()
            .contains("budget"));
        let mut b = collection();
        b[76..78].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(collection_index(&mut Cursor::new(b), "FacePS").is_err());
    }
}
