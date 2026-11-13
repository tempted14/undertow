use super::*;
use crate::discovery::{self, Feed, Section};

impl Tidal {
    pub(crate) fn home(&mut self) -> Result<Feed> {
        ensure!(
            self.session.compatibility,
            "Recommendations require the TIDAL web connection"
        );
        let document = match self.discovery_document("../v2/home/feed/static") {
            Ok(v) => v,
            Err(error)
                if ["HTTP 400", "HTTP 404", "HTTP 405"]
                    .iter()
                    .any(|s| error.to_string().contains(s)) =>
            {
                self.discovery_document("pages/home")?
            }
            Err(error) => return Err(error),
        };
        let feed = discovery::feed(&document);
        ensure!(
            !feed.sections.is_empty(),
            "TIDAL returned no recommendations. Try again shortly."
        );
        // A selectedAlbumCoverFallback is valid for a Home tile, but must never
        // replace a real artist portrait in the shared artist-page hint cache.
        for module in document["items"].as_array().into_iter().flatten() {
            for entry in module["items"].as_array().into_iter().flatten() {
                if entry["type"] == "ARTIST" {
                    self.remember_artist(&entry["data"]);
                }
            }
        }
        Ok(feed)
    }
    pub(crate) fn discovery_section(&mut self, source: &Section) -> Result<Section> {
        ensure!(
            self.session.compatibility,
            "Recommendations require the TIDAL web connection"
        );
        let mut section = source.clone();
        if let Some(path) = &source.more {
            let route =
                discovery::page_route(path).context("Unrecognized TIDAL recommendation page")?;
            let document = self.discovery_document(&route)?;
            let parsed = if document.get("rows").is_some() {
                discovery::feed(&document)
                    .sections
                    .into_iter()
                    .next()
                    .context("TIDAL returned an empty section")?
            } else {
                discovery::section(&document, &source.id)
            };
            ensure!(
                !parsed.items.is_empty(),
                "TIDAL returned an empty section. Try again shortly."
            );
            section.items = parsed.items;
            section.more = None;
        }
        Ok(section)
    }
    pub(crate) fn mix_tracks(&mut self, id: &str) -> Result<Vec<Track>> {
        valid_id(id)?;
        ensure!(
            self.session.compatibility,
            "Mixes require the TIDAL web connection"
        );
        let page = self
            .discovery_document(&format!("pages/mix?mixId={id}"))
            .context("Mix header")?;
        for row in page["rows"].as_array().into_iter().flatten() {
            for module in row["modules"].as_array().into_iter().flatten() {
                if module["type"] != "TRACK_LIST" {
                    continue;
                }
                let parsed = discovery::section(module, "mix-tracks");
                let complete = module
                    .pointer("/pagedList/totalNumberOfItems")
                    .and_then(Value::as_u64)
                    .is_some_and(|total| total <= parsed.items.len() as u64);
                if !complete
                    && let Some(path) = module
                        .pointer("/pagedList/dataApiPath")
                        .and_then(Value::as_str)
                {
                    let route =
                        discovery::page_route(path).context("Unrecognized TIDAL mix page")?;
                    return Ok(self
                        .v1_pages(
                            &route,
                            &[
                                ("deviceType", "BROWSER".into()),
                                ("locale", "en_US".into()),
                                ("platform", "WEB".into()),
                            ],
                        )
                        .context("Mix item page")?
                        .iter()
                        .filter_map(|v| discovery::item(v, "TRACK"))
                        .filter_map(|i| i.catalog.track)
                        .collect());
                }
                return Ok(parsed
                    .items
                    .iter()
                    .filter_map(|i| i.catalog.track.clone())
                    .collect());
            }
        }
        bail!("TIDAL returned no tracks for this mix")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };

    fn mock(responses: Vec<Value>) -> (Tidal, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for body in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                requests.push(String::from_utf8(request).unwrap());
                let body = body.to_string();
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            requests
        });
        let mut tidal = Tidal::new(
            Session {
                access_token: "test-only".into(),
                refresh_token: String::new(),
                expires_at: u64::MAX,
                user_id: 1,
                country: "CA".into(),
                compatibility: true,
                client_id: "test-only".into(),
            },
            Settings::default(),
        )
        .unwrap();
        tidal.test_base = Some(base);
        (tidal, server)
    }
    #[test]
    fn home_and_view_all_use_real_feed_and_required_client_header() {
        let (mut t, server) = mock(vec![
            json!({"items":[{"moduleId":"ALBUM_RECOMMENDATIONS","title":"For you","type":"HORIZONTAL_LIST","viewAll":"home/pages/ALBUM_RECOMMENDATIONS/view-all","items":[{"type":"ALBUM","data":{"id":42,"title":"Suggested"}}]}]}),
            json!({"items":[{"type":"ALBUM","data":{"id":43,"title":"More music"}}]}),
        ]);
        let feed = t.home().unwrap();
        let expanded = t.discovery_section(&feed.sections[0]).unwrap();
        assert_eq!(expanded.id, "ALBUM_RECOMMENDATIONS");
        assert_eq!(expanded.items[0].catalog.id, "43");
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /v2/home/feed/static?"));
        assert!(requests[0].contains("x-tidal-client-version: 2025.7.16"));
        assert!(requests[1].starts_with("GET /v2/home/pages/ALBUM_RECOMMENDATIONS/view-all?"));
        assert!(requests.iter().all(|r| !r.contains("favorites")));
    }
    #[test]
    fn complete_mix_does_not_follow_a_non_paged_module_data_link() {
        let (mut t, server) = mock(vec![
            json!({"rows":[{"modules":[{"type":"TRACK_LIST","supportsPaging":false,"pagedList":{"totalNumberOfItems":1,"dataApiPath":"pages/data/non-paged","items":[{"id":4,"title":"Complete mix"}]}}]}]}),
        ]);
        assert_eq!(t.mix_tracks("abc").unwrap()[0].id, 4);
        assert_eq!(server.join().unwrap().len(), 1);
    }
    #[test]
    fn mix_fetches_complete_track_page_instead_of_only_the_ten_track_preview() {
        let preview = json!({"rows":[{"modules":[{"type":"TRACK_LIST","pagedList":{"dataApiPath":"pages/data/mix-items?mixId=abc","totalNumberOfItems":2,"items":[{"id":1,"title":"Preview"}]}}]}]});
        let full = json!({"totalNumberOfItems":2,"items":[{"type":"TRACK","item":{"id":1,"title":"First"}},{"type":"TRACK","item":{"id":2,"title":"Second"}}]});
        let (mut t, server) = mock(vec![preview, full]);
        let tracks = t.mix_tracks("abc").unwrap();
        assert_eq!(tracks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![1, 2]);
        let requests = server.join().unwrap();
        assert!(requests[0].contains("mixId=abc"));
        assert!(requests[1].starts_with("GET /v1/pages/data/mix-items?mixId=abc&"));
    }
}
