use crate::model::Track;
use anyhow::{Result, ensure};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

// FLAC metadata is rewritten in bounded blocks. Encoded audio frames are copied unchanged.
pub fn write_flac(path: &Path, track: &Track, cover: Option<&[u8]>) -> Result<()> {
    let temp = path.with_extension("tagging.part");
    let result = (|| -> Result<()> {
        let mut input = File::open(path)?;
        let mut magic = [0; 4];
        input.read_exact(&mut magic)?;
        ensure!(
            &magic == b"fLaC",
            "Native tag writing requires a raw FLAC container"
        );
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        output.write_all(b"fLaC")?;
        let mut metadata_total = 0usize;
        loop {
            let mut header = [0; 4];
            input.read_exact(&mut header)?;
            let last = header[0] & 0x80 != 0;
            let kind = header[0] & 0x7f;
            let len = u32::from_be_bytes([0, header[1], header[2], header[3]]) as usize;
            metadata_total += len;
            ensure!(
                metadata_total <= 16 * 1024 * 1024,
                "FLAC metadata exceeds the 16 MiB safety budget"
            );
            let mut block = vec![0; len];
            input.read_exact(&mut block)?;
            if kind != 4 && kind != 6 && kind != 1 {
                write_block(&mut output, kind, false, &block)?;
            }
            if last {
                break;
            }
        }
        let vendor = b"Undertow native FLAC tags";
        let fields = [
            format!("TITLE={}", track.title),
            format!("ARTIST={}", track.artist),
            format!("ALBUM={}", track.album),
            format!("ISRC={}", track.isrc),
            format!("TIDAL_TRACK_ID={}", track.id),
            format!("URL=https://tidal.com/browse/track/{}", track.id),
        ];
        let mut comments = Vec::new();
        comments.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        comments.extend_from_slice(vendor);
        comments.extend_from_slice(&(fields.len() as u32).to_le_bytes());
        for field in fields {
            comments.extend_from_slice(&(field.len() as u32).to_le_bytes());
            comments.extend_from_slice(field.as_bytes());
        }
        write_block(&mut output, 4, cover.is_none(), &comments)?;
        if let Some(cover) = cover {
            ensure!(
                cover.len() <= 8 * 1024 * 1024,
                "Cover exceeds the 8 MiB tag budget"
            );
            let mut picture = Vec::with_capacity(cover.len() + 64);
            picture.extend_from_slice(&3u32.to_be_bytes());
            let mime = b"image/jpeg";
            picture.extend_from_slice(&(mime.len() as u32).to_be_bytes());
            picture.extend_from_slice(mime);
            picture.extend_from_slice(&0u32.to_be_bytes());
            for _ in 0..4 {
                picture.extend_from_slice(&0u32.to_be_bytes());
            }
            picture.extend_from_slice(&(cover.len() as u32).to_be_bytes());
            picture.extend_from_slice(cover);
            write_block(&mut output, 6, true, &picture)?;
        }
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        drop(input);
        drop(output);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
fn write_block(out: &mut impl Write, kind: u8, last: bool, data: &[u8]) -> Result<()> {
    ensure!(
        data.len() < 1 << 24,
        "FLAC metadata block exceeds the format limit"
    );
    let n = data.len() as u32;
    out.write_all(&[
        kind | if last { 0x80 } else { 0 },
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
    ])?;
    out.write_all(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tagging_preserves_encoded_frames() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("undertow-tags-{}", crate::auth::random_urlsafe()));
        std::fs::create_dir_all(&root).unwrap();
        let p = root.join("test.flac");
        let mut bytes = b"fLaC".to_vec();
        bytes.extend_from_slice(&[0x80, 0, 0, 34]);
        bytes.extend_from_slice(&[0; 34]);
        bytes.extend_from_slice(b"encoded audio unchanged");
        std::fs::write(&p, bytes).unwrap();
        write_flac(
            &p,
            &Track {
                title: "Test".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let after = std::fs::read(&p).unwrap();
        assert!(after.ends_with(b"encoded audio unchanged"));
        assert!(after.windows(10).any(|w| w == b"TITLE=Test"));
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_dir(root);
    }
}
