use super::config::Config;
use super::Result;
use chrono::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use ureq::Agent;
use url::Url;

const COOLDOWN: i64 = 30;

pub fn run() -> Result<()> {
    let config = Config::load_default()?;
    let mut sock = dqtt::Client::connect(&config.sock_path);
    sock.subscribe(b"switch")?;
    let mut lcd = LcdWriter::new(&config.lcd_path)?;
    let mut channel = super::hls::expect_channel(sock.wait(0)?).unwrap();
    let mut meta = MetaClient::new(&config, channel);
    lcd.write_name(&meta.name)?;

    loop {
        meta.update();
        lcd.write_title(&meta.get_title())?;

        match super::hls::expect_channel(sock.wait(500)?) {
            Some(0) => {
                lcd.standby()?;
                break;
            }
            Some(new) if new != channel => {
                channel = new;
                meta = MetaClient::new(&config, channel);
                lcd.write_name(&meta.name)?;
            }
            _ => {}
        }
    }

    Ok(())
}

struct LcdWriter {
    file: File,
    buf: String,
    len: usize,
    offset: usize,
}

impl LcdWriter {
    fn new(path: &str) -> Result<Self> {
        Ok(Self {
            file: std::fs::OpenOptions::new().write(true).open(path)?,
            buf: String::new(),
            len: 0,
            offset: 0,
        })
    }

    fn write_name(&mut self, name: &str) -> Result<()> {
        let n = format!("{:^16}", name);
        self.file
            .write_all(&[b"\x1b[2J", n.as_bytes(), b"\n"].concat())?;
        Ok(())
    }

    fn write_title(&mut self, title: &str) -> Result<()> {
        if self.buf != title {
            self.buf = title.to_owned();
            self.len = self.buf.chars().count();
            self.offset = 0;
        }

        for c in self.buf.chars().skip(self.offset).take(16) {
            if c >= ' ' {
                write!(&mut self.file, "{}", c)?;
            } else {
                write!(&mut self.file, "?")?;
            }
        }

        self.file.write_all(b"\n")?;

        if self.len > 16 {
            self.offset += 1;
            self.offset %= self.len - 15;

            if self.offset == 0 || self.offset == 1 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        Ok(())
    }

    fn standby(&mut self) -> Result<()> {
        self.file.write_all(b"\x04")?;
        Ok(())
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct JsonData {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    title: String,
    performer: Option<String>,
}

impl Default for JsonData {
    fn default() -> Self {
        Self {
            start_time: Default::default(),
            end_time: Default::default(),
            title: "".to_string(),
            performer: None,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
struct OuterJson {
    data: JsonData,
}

enum Status {
    Pending,
    Current,
    Stale,
}

use Status::*;

#[derive(Clone, Debug)]
struct MetaData {
    data: JsonData,
    last_fetch: DateTime<Utc>,
    url: Url,
}

impl MetaData {
    fn new(url_str: &str, params: &HashMap<String, String>) -> Self {
        Self {
            data: Default::default(),
            last_fetch: Default::default(),
            url: Url::parse_with_params(url_str, params).unwrap(),
        }
    }

    fn fetch(&mut self, agent: &mut Agent) -> Result<()> {
        if Utc::now() - self.last_fetch > chrono::Duration::seconds(COOLDOWN) {
            self.last_fetch = Utc::now();
            let text = agent.request_url("GET", &self.url).call()?.into_string()?;
            let outer: OuterJson = serde_json::from_str(&text)?;
            self.data = outer.data;
        }

        Ok(())
    }

    fn status(&self) -> Status {
        let now = Utc::now();

        match (self.data.start_time <= now, self.data.end_time <= now) {
            (false, false) => Pending,
            (true, false) => Current,
            (_, true) => Stale,
        }
    }

    fn is_current(&self) -> bool {
        matches!(self.status(), Current)
    }

    fn is_stale(&self) -> bool {
        matches!(self.status(), Stale)
    }
}

impl std::fmt::Display for MetaData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(perf) = &self.data.performer {
            write!(f, "{} -- {}", &self.data.title, &perf)
        } else {
            write!(f, "{}", &self.data.title)
        }
    }
}

pub struct MetaClient {
    agent: Agent,
    name: String,
    md1: MetaData,
    md2: MetaData,
}

impl MetaClient {
    pub fn new(config: &Config, channel: usize) -> Self {
        let params = config.get_meta_params(channel);
        let agent = ureq::builder()
            .user_agent(&config.user_agent)
            .https_only(true)
            .build();

        Self {
            agent,
            name: config.get_channel(channel).name.clone(),
            md1: MetaData::new(&config.meta_url1, &params),
            md2: MetaData::new(&config.meta_url2, &params),
        }
    }

    fn update(&mut self) {
        if self.md1.is_stale() {
            let _ = self.md1.fetch(&mut self.agent);
        }

        if self.md2.is_stale() {
            let _ = self.md2.fetch(&mut self.agent);
        }
    }

    fn get_title(&self) -> String {
        match (self.md1.is_current(), self.md2.is_current()) {
            (true, true) => format!("{} {}", &self.md1, &self.md2),
            (true, false) => format!("{}", &self.md1),
            (false, true) => format!("{}", &self.md2),
            (false, false) => "".to_string(),
        }
    }
}
