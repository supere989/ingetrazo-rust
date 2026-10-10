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
                    return Ok(index as u32);
                }
            }
        }
    }
    Err(format!(
        "Native PostScript identity {name:?} not found in collection"
    ))
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
        assert!(
            collection_index(&mut Cursor::new(b), "FacePS")
                .unwrap_err()
                .contains("budget")
        );
        let mut b = collection();
        b[76..78].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(collection_index(&mut Cursor::new(b), "FacePS").is_err());
    }
}
