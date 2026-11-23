use super::{Line, Lyrics};
use anyhow::{Result, ensure};
use quick_xml::{Reader, XmlVersion, escape::resolve_xml_entity, events::Event};

pub(super) fn parse(text: &str) -> Result<Lyrics> {
    ensure!(text.len() <= 1024 * 1024, "TTML lyrics are too large");
    let mut reader = Reader::from_str(text);
    let mut lyrics = Lyrics::default();
    let mut depth = 0usize;
    let mut skip = None;
    let mut line = None;
    let append = |line: &mut Option<(usize, Line)>, text: &str| {
        if let Some((_, line)) = line {
            line.text.push_str(text);
        }
    };

    loop {
        match reader.read_event()? {
            Event::Start(element) => {
                depth += 1;
                ensure!(depth <= 64, "TTML lyrics are nested too deeply");
                if skip.is_some() {
                    continue;
                }
                let mut begin = None;
                for attribute in element.attributes() {
                    let attribute = attribute?;
                    match attribute.key.local_name().as_ref() {
                        b"begin" => {
                            begin = time(&attribute.decoded_and_normalized_value(
                                XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )?)
                        }
                        b"role"
                            if matches!(
                                attribute.value.as_ref(),
                                b"x-bg" | b"x-translation" | b"x-roman"
                            ) =>
                        {
                            skip = Some(depth);
                        }
                        _ => {}
                    }
                }
                if skip.is_some() {
                    continue;
                }
                match element.local_name().as_ref() {
                    b"p" if line.is_none() => {
                        line = Some((
                            depth,
                            Line {
                                start: begin.unwrap_or(f64::NAN),
                                ..Default::default()
                            },
                        ));
                    }
                    b"span" => {
                        if let Some((_, line)) = line.as_mut()
                            && !line.start.is_finite()
                            && let Some(start) = begin
                        {
                            line.start = start;
                        }
                    }
                    _ => {}
                }
            }
            Event::End(_) => {
                if let Some(skipped_at) = skip {
                    if skipped_at == depth {
                        skip = None;
                    }
                } else if line.as_ref().is_some_and(|(at, _)| *at == depth) {
                    let (_, mut line) = line.take().unwrap();
                    if line.start.is_finite() && !line.text.trim().is_empty() {
                        line.text = line.text.trim().to_owned();
                        lyrics.lines.push(line);
                        ensure!(lyrics.lines.len() <= 4000, "Too many TTML lyric lines");
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Event::Text(text) if skip.is_none() && line.is_some() => {
                append(&mut line, &text.decode()?);
            }
            Event::CData(text) if skip.is_none() && line.is_some() => {
                append(&mut line, &text.decode()?);
            }
            Event::GeneralRef(reference) if skip.is_none() && line.is_some() => {
                if let Some(ch) = reference.resolve_char_ref()? {
                    append(&mut line, ch.encode_utf8(&mut [0; 4]));
                } else if let Some(text) = resolve_xml_entity(&reference.decode()?) {
                    append(&mut line, text);
                }
            }
            Event::Empty(element) if element.local_name().as_ref() == b"br" && skip.is_none() => {
                append(&mut line, "\n");
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(!lyrics.lines.is_empty(), "No timed TTML lines found");
    lyrics.lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    lyrics.synced = true;
    Ok(lyrics)
}

fn time(value: &str) -> Option<f64> {
    let value = value.trim();
    let seconds = if let Some((head, seconds)) = value.rsplit_once(':') {
        let seconds = seconds
            .parse::<f64>()
            .ok()
            .filter(|s| (0. ..60.).contains(s))?;
        let minutes = if let Some((hours, minutes)) = head.split_once(':') {
            (hours.parse::<u32>().ok()? as f64 * 60.) + minutes.parse::<u32>().ok()? as f64
        } else {
            head.parse::<u32>().ok()? as f64
        };
        minutes * 60. + seconds
    } else if let Some(milliseconds) = value.strip_suffix("ms") {
        milliseconds.parse::<f64>().ok()? / 1000.
    } else {
        value
            .strip_suffix('s')
            .unwrap_or(value)
            .parse::<f64>()
            .ok()?
    };
    (seconds.is_finite() && seconds >= 0.).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_legacy_timed_spans_but_displays_complete_lines() {
        let lyrics = parse(r#"<tt xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><div>
            <p begin="0.115"><span begin="0.115" end="250ms">Em</span><span begin="0.250s" end="00:00:00.640">bers &amp; glow</span><span ttm:role="x-bg"><span begin="0.3" end="0.4">echo</span></span></p>
            <p><span begin="0:02.25" end="0:02.65">Soft</span> tides<span ttm:role="x-translation">translation</span></p>
            </div></body></tt>"#).unwrap();
        assert_eq!(lyrics.lines[0].text, "Embers & glow");
        assert_eq!(lyrics.lines[0].start, 0.115);
        assert_eq!(lyrics.lines[1].text, "Soft tides");
        assert_eq!(lyrics.active(2.3), Some(1));
        assert_eq!(lyrics.next_line_change(0.5), Some(1.75));
    }
}
