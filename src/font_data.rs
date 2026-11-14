//! Keep installed font bytes file-backed instead of committing a second font file
//! in the process heap. Every existing glyph and fallback font remains available.
pub enum FontBytes {
    #[cfg(windows)]
    Mapped {
        bytes: memmap2::Mmap,
        _file: std::fs::File,
    },
    Owned(Box<[u8]>),
}
impl FontBytes {
    pub fn open(path: &std::path::Path) -> Option<Self> {
        if std::fs::metadata(path).ok()?.len() > 20 * 1024 * 1024 {
            return None;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Deny write/delete sharing for the entire mapping lifetime. If an
            // existing writer prevents that lock, retain the original owned copy.
            if let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(path)
            {
                // SAFETY: this read-only handle denies writers and deletion and is
                // kept alive with the mapping; consumers only receive shared bytes.
                if let Ok(bytes) = unsafe { memmap2::MmapOptions::new().map(&file) } {
                    return Some(Self::Mapped { bytes, _file: file });
                }
            }
        }
        std::fs::read(path)
            .ok()
            .map(|bytes| Self::Owned(bytes.into_boxed_slice()))
    }
    pub fn as_slice(&self) -> &[u8] {
        match self {
            #[cfg(windows)]
            Self::Mapped { bytes, .. } => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn mapped_font_bytes_are_identical_and_cannot_be_changed_under_the_reader() {
        let path =
            std::env::temp_dir().join(format!("undertow-font-map-{}.bin", rand::random::<u64>()));
        let original = vec![13u8; 65_536];
        std::fs::write(&path, &original).unwrap();
        let font = super::FontBytes::open(&path).unwrap();
        assert!(matches!(font, super::FontBytes::Mapped { .. }));
        assert_eq!(font.as_slice(), original);
        assert!(std::fs::write(&path, [7u8]).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(font);
        std::fs::remove_file(path).unwrap();
    }
}
