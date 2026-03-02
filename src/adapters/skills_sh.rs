use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde::Deserialize;

const BASE_URL: &str = "https://skills.sh";

#[derive(Clone, Copy, Debug)]
pub enum SkillsMode {
    AllTime,
    Trending,
    Hot,
}

impl SkillsMode {
    fn as_str(self) -> &'static str {
        match self {
            SkillsMode::AllTime => "all-time",
            SkillsMode::Trending => "trending",
            SkillsMode::Hot => "hot",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SkillListItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "skillId")]
    pub skill_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub installs: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CatalogPage {
    pub skills: Vec<SkillListItem>,
    pub total: u64,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    pub page: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchResponse {
    pub skills: Vec<SkillListItem>,
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetailSource {
    Rsc,
    HtmlFallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityAudit {
    pub slug: String,
    pub name: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledOn {
    pub agent: String,
    pub installs: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillDetail {
    pub slug: String,
    pub weekly_installs: String,
    pub repository: String,
    pub github_stars: String,
    pub first_seen: String,
    pub security_audits: Vec<SecurityAudit>,
    pub installed_on: Vec<InstalledOn>,
    pub source: DetailSource,
}

pub struct SkillsShClient {
    http: Client,
}

impl SkillsShClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("skills-tui/0.1 (+https://github.com/alpha-innovation-labs/skills-tui)")
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http })
    }

    pub fn fetch_catalog_page(&self, mode: SkillsMode, page: u64) -> Result<CatalogPage> {
        let url = format!("{BASE_URL}/api/skills/{}/{}", mode.as_str(), page);
        self.http
            .get(url)
            .send()
            .context("catalog request failed")?
            .error_for_status()
            .context("catalog request returned non-success status")?
            .json::<CatalogPage>()
            .context("failed to decode catalog payload")
    }

    pub fn fetch_search(&self, query: &str, limit: u64) -> Result<SearchResponse> {
        let url = format!("{BASE_URL}/api/search");
        self.http
            .get(url)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .context("search request failed")?
            .error_for_status()
            .context("search request returned non-success status")?
            .json::<SearchResponse>()
            .context("failed to decode search payload")
    }

    pub fn fetch_skill_detail(&self, owner: &str, repo: &str, skill: &str) -> Result<SkillDetail> {
        let slug = format!("{owner}/{repo}/{skill}");
        let rsc_url = format!("{BASE_URL}/{slug}.rsc");
        self.fetch_skill_detail_with_rsc_url(owner, repo, skill, &rsc_url)
    }

    pub fn fetch_skill_detail_with_rsc_url(
        &self,
        owner: &str,
        repo: &str,
        skill: &str,
        rsc_url: &str,
    ) -> Result<SkillDetail> {
        let slug = format!("{owner}/{repo}/{skill}");

        let rsc_response = self
            .http
            .get(rsc_url)
            .header("Accept", "text/x-component")
            .send()
            .ok()
            .and_then(|resp| resp.error_for_status().ok())
            .and_then(|resp| resp.text().ok());

        if let Some(rsc_response) = rsc_response {
            if let Ok(detail) = parse_rsc_detail(&slug, &rsc_response) {
                return Ok(detail);
            }
        }

        let html_url = format!("{BASE_URL}/{slug}");
        let html = self
            .http
            .get(&html_url)
            .send()
            .context("detail HTML request failed")?
            .error_for_status()
            .context("detail HTML request returned non-success status")?
            .text()
            .context("failed to read detail HTML response")?;

        parse_html_detail(&slug, &html)
    }
}

fn parse_rsc_detail(slug: &str, payload: &str) -> Result<SkillDetail> {
    let weekly_installs = extract_labeled_value(payload, "Weekly Installs")
        .ok_or_else(|| anyhow!("missing weekly installs in RSC payload"))?;
    let github_stars = extract_labeled_value(payload, "GitHub Stars")
        .ok_or_else(|| anyhow!("missing GitHub stars in RSC payload"))?;
    let first_seen = extract_labeled_value(payload, "First Seen")
        .ok_or_else(|| anyhow!("missing first seen in RSC payload"))?;

    let repository = Regex::new(r#"href":"https://github.com/([^"]+)"#)
        .expect("valid repository regex")
        .captures(payload)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| anyhow!("missing repository in RSC payload"))?;

    let security_re = Regex::new(
        r#"(?s)href":"/[^"]*/security/([^"]+)".*?"children":"([^"]+)".*?"children":"(Pass|Warn|Fail)"#,
    )
    .expect("valid security regex");
    let security_audits = security_re
        .captures_iter(payload)
        .filter_map(|caps| {
            Some(SecurityAudit {
                slug: caps.get(1)?.as_str().to_string(),
                name: caps.get(2)?.as_str().to_string(),
                status: caps.get(3)?.as_str().to_string(),
            })
        })
        .collect::<Vec<_>>();

    let installed_on_re = Regex::new(
        r#"(?s)\$","div","([^"]+)",\{"className":"flex items-center justify-between text-sm py-2".*?"children":"([0-9.]+[KMB]?)"#,
    )
    .expect("valid installed-on regex");
    let installed_on = installed_on_re
        .captures_iter(payload)
        .filter_map(|caps| {
            Some(InstalledOn {
                agent: caps.get(1)?.as_str().to_string(),
                installs: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect::<Vec<_>>();

    Ok(SkillDetail {
        slug: slug.to_string(),
        weekly_installs,
        repository,
        github_stars,
        first_seen,
        security_audits,
        installed_on,
        source: DetailSource::Rsc,
    })
}

fn parse_html_detail(slug: &str, payload: &str) -> Result<SkillDetail> {
    let weekly_installs = extract_between(payload, "Weekly Installs</span></div><div", "</div>")
        .map(strip_html)
        .ok_or_else(|| anyhow!("missing weekly installs in HTML payload"))?;
    let repository = Regex::new(r#"title="([^"]+/[^"]+)"#)
        .expect("valid repository title regex")
        .captures(payload)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| anyhow!("missing repository in HTML payload"))?;
    let github_stars = extract_between(payload, "GitHub Stars</span></div><div", "</div>")
        .map(strip_html)
        .ok_or_else(|| anyhow!("missing stars in HTML payload"))?;
    let first_seen = extract_between(payload, "First Seen</span></div><div", "</div>")
        .map(strip_html)
        .ok_or_else(|| anyhow!("missing first seen in HTML payload"))?;

    let security_re = Regex::new(
        r#"(?s)href="/[^"]*/security/([^"]+)"[^>]*>.*?truncate">([^<]+)</span><span[^>]*>(Pass|Warn|Fail)</span>"#,
    )
    .expect("valid HTML security regex");
    let security_audits = security_re
        .captures_iter(payload)
        .filter_map(|caps| {
            Some(SecurityAudit {
                slug: caps.get(1)?.as_str().to_string(),
                name: caps.get(2)?.as_str().to_string(),
                status: caps.get(3)?.as_str().to_string(),
            })
        })
        .collect::<Vec<_>>();

    let installed_on_re = Regex::new(
        r#"class="text-foreground">([^<]+)</span><span class="text-muted-foreground font-mono">([0-9.]+[KMB]?)</span>"#,
    )
    .expect("valid HTML installed-on regex");
    let installed_on = installed_on_re
        .captures_iter(payload)
        .filter_map(|caps| {
            Some(InstalledOn {
                agent: caps.get(1)?.as_str().to_string(),
                installs: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect::<Vec<_>>();

    Ok(SkillDetail {
        slug: slug.to_string(),
        weekly_installs,
        repository,
        github_stars,
        first_seen,
        security_audits,
        installed_on,
        source: DetailSource::HtmlFallback,
    })
}

fn extract_labeled_value(payload: &str, label: &str) -> Option<String> {
    let marker = format!("\"children\":\"{label}\"");
    let idx = payload.find(&marker)?;
    let tail = &payload[idx + marker.len()..];
    let value_re = Regex::new(r#"children":"([^"]+)"#).expect("valid children regex");

    for caps in value_re.captures_iter(tail) {
        let value = caps.get(1)?.as_str();
        if value == label {
            continue;
        }
        if value.chars().any(|ch| ch.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }

    None
}

fn extract_between(payload: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = payload.find(start)? + start.len();
    let tail = &payload[start_idx..];
    let end_idx = tail.find(end)?;
    Some(tail[..end_idx].to_string())
}

fn strip_html(input: String) -> String {
    Regex::new(r"<[^>]+>")
        .expect("valid html-strip regex")
        .replace_all(&input, "")
        .to_string()
        .trim()
        .to_string()
}
