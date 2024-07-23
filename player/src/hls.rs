use super::config::Config;
use super::error::Error;
use super::Result;
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use io::{Read, Write};
use m3u8_rs::Playlist;
use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::{io, thread};
use ureq::Agent;
use url::{ParseError, Url};

const DL_ESTIMATE: usize = 1024 * 200;
const DL_LIMIT: u64 = 1024 * 1000 * 50;
const TIMEOUT: u64 = 10;
const MAX_ERRS: i32 = 4;

pub fn play() -> Result<()> {
    let config = Config::load_default()?;
    let mut sock = dqtt::Client::connect(&config.sock_path);
    sock.subscribe(b"switch")?;
    let mut errs = 0;

    loop {
        let channel = expect_channel(sock.wait(0)?).unwrap();
        let mut hls = HlsClient::new(&config)?;
        hls.change_channel(&config, channel)?;

        loop {
            match hls.playing() {
                Ok(true) => errs = 0,
                Ok(false) => break,
                Err(e) => {
                    eprintln!("{}", &e);
                    errs += 1;

                    if errs > MAX_ERRS {
                        sock.publish(b"system", b"network_error=true")?;
                    }

                    thread::sleep(std::time::Duration::from_secs(5));
                }
            }

            match expect_channel(sock.wait(-1)?) {
                Some(0) => break,
                Some(new) => {
                    sock.publish(b"playback", b"restart")?;
                    hls.change_channel(&config, new)?;
                }
                _ => {}
            }
        }

        sock.publish(b"playback", b"stop")?;
    }
}

pub fn expect_channel(s: Option<&[u8]>) -> Option<usize> {
    match s {
        Some(b"channel=0") => Some(0),
        Some(b"channel=1") => Some(1),
        Some(b"channel=2") => Some(2),
        Some(b"channel=3") => Some(3),
        Some(b"channel=4") => Some(4),
        _ => None,
    }
}

fn run_audio_thread(sock_path: String, receiver: Receiver<Vec<u8>>) {
    let mut sock = dqtt::Client::connect(&sock_path);
    sock.subscribe(b"playback").unwrap();

    'outer: loop {
        let mut mpv = Command::new("mpv")
            .arg("--quiet")
            .arg("--idle=yes")
            .arg("-")
            .stdin(Stdio::piped())
            .spawn()
            .expect("failed to launch mpv");

        let mut stdin = mpv.stdin.take().expect("failed to take stdin");

        'inner: loop {
            match receiver.try_recv() {
                Ok(seg) => stdin
                    .write_all(seg.as_slice())
                    .expect("failed to write to stdin"),
                Err(TryRecvError::Empty) => {}
                _ => {
                    let _ = mpv.kill();
                    break 'outer;
                }
            }

            match sock.wait(50).unwrap() {
                Some(b"restart") => {
                    let _ = mpv.kill();

                    for _ in 0..receiver.len() {
                        let _ = receiver.try_recv();
                    }

                    break 'inner;
                }
                Some(b"stop") => {
                    let _ = mpv.kill();
                    break 'outer;
                }
                _ => {}
            }
        }
    }
}

struct HlsClient {
    agent: Agent,
    end_list: bool,
    media_url: Url,
    segments: VecDeque<Url>,
    sender: Sender<Vec<u8>>,
    seq: u64,
}

impl HlsClient {
    fn new(config: &Config) -> Result<Self> {
        let agent = ureq::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT))
            .user_agent(&config.user_agent)
            .https_only(true)
            .build();
        let (sender, receiver) = bounded(config.queue_length);
        let sp = config.sock_path.clone();
        let audio = thread::spawn(move || run_audio_thread(sp, receiver));
        assert!(!audio.is_finished());

        Ok(Self {
            agent,
            end_list: false,
            media_url: "http://localhost".parse()?,
            segments: VecDeque::new(),
            sender,
            seq: 0,
        })
    }

    fn change_channel(&mut self, config: &Config, channel: usize) -> Result<()> {
        let channel = config.get_channel(channel);
        let url = channel.manifest_url.parse()?;
        let text = self.agent.request_url("GET", &url).call()?.into_string()?;

        let media_url = match m3u8_rs::parse_playlist(text.as_bytes()) {
            Ok((_, Playlist::MasterPlaylist(pl))) => pl
                .variants
                .iter()
                .min_by_key(|v| v.bandwidth.abs_diff(config.target_bandwidth))
                .ok_or(Error::NoVariantStream)?
                .uri
                .parse()?,
            Ok((_, Playlist::MediaPlaylist(_))) => url,
            _ => return Err(Box::new(Error::ParseError)),
        };

        self.end_list = false;
        self.media_url = media_url;
        self.segments.clear();
        self.seq = 0;
        Ok(())
    }

    fn push_segment(&mut self, url_str: &str) -> Result<()> {
        let url = match url_str.parse() {
            Err(ParseError::RelativeUrlWithoutBase)
            | Err(ParseError::RelativeUrlWithCannotBeABaseBase) => self.media_url.join(url_str),
            any => any,
        }?;

        self.segments.push_back(url);
        Ok(())
    }

    fn fetch_playlist(&mut self) -> Result<()> {
        let text = self
            .agent
            .request_url("GET", &self.media_url)
            .call()?
            .into_string()?;
        let (_, pl) = m3u8_rs::parse_media_playlist(text.as_bytes())
            .map_err(|_| Box::new(Error::ParseError))?;
        let mut seq = pl.media_sequence;

        for s in pl.segments.iter() {
            if s.discontinuity {
                self.end_list = true;
                return Ok(());
            }

            if seq >= self.seq {
                self.push_segment(&s.uri)?;
            }

            seq += 1;
        }

        self.seq = seq;
        self.end_list = pl.end_list;
        Ok(())
    }

    fn fetch_segment(&mut self) -> Result<Vec<u8>> {
        let url = self.segments.pop_front().unwrap();
        let mut buf = Vec::with_capacity(DL_ESTIMATE);
        self.agent
            .request_url("GET", &url)
            .call()?
            .into_reader()
            .take(DL_LIMIT)
            .read_to_end(&mut buf)?;

        if buf.is_empty() {
            Err(Box::new(Error::EmptySegment))
        } else {
            Ok(buf)
        }
    }

    fn playing(&mut self) -> Result<bool> {
        Ok(match (self.segments.is_empty(), self.end_list) {
            (true, true) => false,
            (true, false) => {
                self.fetch_playlist()?;
                true
            }
            (false, _) => {
                let seg = self.fetch_segment()?;
                self.sender.send(seg)?;
                true
            }
        })
    }
}
