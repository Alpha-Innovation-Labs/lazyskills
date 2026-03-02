use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::services::skills_cache;

const BASE_URL: &str = "https://skills.sh";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogPage {
    pub skills: Vec<SkillListItem>,
    pub total: u64,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    pub page: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub skills: Vec<SkillListItem>,
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DetailSource {
    Rsc,
    HtmlFallback,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecurityAudit {
    pub slug: String,
    pub name: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstalledOn {
    pub agent: String,
    pub installs: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

    pub fn fetch_catalog_page_cached_swr(
        &self,
        mode: SkillsMode,
        page: u64,
    ) -> Result<CatalogPage> {
        let key = format!("catalog:{}:{}", mode.as_str(), page);
        if let Some(cached) = skills_cache::read_json::<CatalogPage>("skills_sh", &key) {
            thread::spawn(move || {
                if let Ok(client) = SkillsShClient::new() {
                    if let Ok(fresh) = client.fetch_catalog_page(mode, page) {
                        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
                    }
                }
            });
            return Ok(cached);
        }

        let fresh = self.fetch_catalog_page(mode, page)?;
        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
        Ok(fresh)
    }

    pub fn fetch_homepage_all_time_leaderboard(&self, limit: usize) -> Result<Vec<SkillListItem>> {
        let html = self
            .http
            .get(BASE_URL)
            .send()
            .context("homepage request failed")?
            .error_for_status()
            .context("homepage request returned non-success status")?
            .text()
            .context("failed to read homepage response")?;

        parse_homepage_leaderboard(&html, limit)
    }

    pub fn fetch_homepage_all_time_leaderboard_cached_swr(
        &self,
        limit: usize,
    ) -> Result<Vec<SkillListItem>> {
        let key = format!("homepage_all_time:{}", limit);
        if let Some(cached) = skills_cache::read_json::<Vec<SkillListItem>>("skills_sh", &key) {
            thread::spawn(move || {
                if let Ok(client) = SkillsShClient::new() {
                    if let Ok(fresh) = client.fetch_homepage_all_time_leaderboard(limit) {
                        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
                    }
                }
            });
            return Ok(cached);
        }

        let fresh = self.fetch_homepage_all_time_leaderboard(limit)?;
        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
        Ok(fresh)
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

    pub fn fetch_search_cached_swr(&self, query: &str, limit: u64) -> Result<SearchResponse> {
        let key = format!("search:{}:{}", query, limit);
        if let Some(cached) = skills_cache::read_json::<SearchResponse>("skills_sh", &key) {
            let query_owned = query.to_string();
            thread::spawn(move || {
                if let Ok(client) = SkillsShClient::new() {
                    if let Ok(fresh) = client.fetch_search(&query_owned, limit) {
                        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
                    }
                }
            });
            return Ok(cached);
        }

        let fresh = self.fetch_search(query, limit)?;
        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
        Ok(fresh)
    }

    pub fn fetch_skill_detail(&self, owner: &str, repo: &str, skill: &str) -> Result<SkillDetail> {
        let slug = format!("{owner}/{repo}/{skill}");
        let rsc_url = format!("{BASE_URL}/{slug}.rsc");
        self.fetch_skill_detail_with_rsc_url(owner, repo, skill, &rsc_url)
    }

    pub fn fetch_skill_detail_cached_swr(
        &self,
        owner: &str,
        repo: &str,
        skill: &str,
    ) -> Result<SkillDetail> {
        let slug = format!("{owner}/{repo}/{skill}");
        let key = format!("detail:{}", slug);
        if let Some(cached) = skills_cache::read_json::<SkillDetail>("skills_sh", &key) {
            let owner_owned = owner.to_string();
            let repo_owned = repo.to_string();
            let skill_owned = skill.to_string();
            thread::spawn(move || {
                if let Ok(client) = SkillsShClient::new() {
                    if let Ok(fresh) =
                        client.fetch_skill_detail(&owner_owned, &repo_owned, &skill_owned)
                    {
                        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
                    }
                }
            });
            return Ok(cached);
        }

        let fresh = self.fetch_skill_detail(owner, repo, skill)?;
        let _ = skills_cache::write_json("skills_sh", &key, &fresh);
        Ok(fresh)
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

fn parse_homepage_leaderboard(payload: &str, limit: usize) -> Result<Vec<SkillListItem>> {
    let row_re = Regex::new(
        r#"(?s)<a[^>]*href="/([^"/]+/[^"/]+/[^"/]+)"[^>]*>.*?<h3[^>]*>([^<]+)</h3>.*?<p[^>]*>([^<]+)</p>.*?<span[^>]*>([0-9][0-9.,]*[KMB]?)</span>.*?</a>"#,
    )
    .expect("valid homepage leaderboard regex");

    let mut rows = Vec::new();
    for caps in row_re.captures_iter(payload).take(limit) {
        let slug = match caps.get(1) {
            Some(m) => m.as_str(),
            None => continue,
        };
        let name = match caps.get(2) {
            Some(m) => m.as_str().trim().to_string(),
            None => continue,
        };
        let source = match caps.get(3) {
            Some(m) => m.as_str().trim().to_string(),
            None => continue,
        };
        let installs = caps
            .get(4)
            .map(|m| parse_compact_number(m.as_str()))
            .transpose()?
            .unwrap_or(0);
        let skill_id = slug.rsplit('/').next().map(|part| part.to_string());

        rows.push(SkillListItem {
            id: Some(slug.to_string()),
            skill_id,
            name,
            source,
            installs,
        });
    }

    if rows.is_empty() {
        return Err(anyhow!("failed to parse homepage leaderboard rows"));
    }
    Ok(rows)
}

fn parse_compact_number(input: &str) -> Result<u64> {
    let normalized = input.trim().replace(',', "");
    if normalized.is_empty() {
        return Ok(0);
    }

    let (number_part, multiplier) = match normalized.chars().last() {
        Some('K') | Some('k') => (&normalized[..normalized.len() - 1], 1_000_f64),
        Some('M') | Some('m') => (&normalized[..normalized.len() - 1], 1_000_000_f64),
        Some('B') | Some('b') => (&normalized[..normalized.len() - 1], 1_000_000_000_f64),
        _ => (normalized.as_str(), 1_f64),
    };

    let value = number_part
        .parse::<f64>()
        .with_context(|| format!("invalid compact number: {input}"))?;
    Ok((value * multiplier).round() as u64)
}

fn parse_html_detail(slug: &str, payload: &str) -> Result<SkillDetail> {
    let weekly_installs = extract_metric_from_html(payload, "Weekly Installs")
        .ok_or_else(|| anyhow!("missing weekly installs in HTML payload"))?;
    let repository = Regex::new(r#"title="([^"]+/[^"]+)"#)
        .expect("valid repository title regex")
        .captures(payload)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .ok_or_else(|| anyhow!("missing repository in HTML payload"))?;
    let github_stars = extract_metric_from_html(payload, "GitHub Stars")
        .ok_or_else(|| anyhow!("missing stars in HTML payload"))?;
    let first_seen = extract_metric_from_html(payload, "First Seen")
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

fn extract_metric_from_html(payload: &str, label: &str) -> Option<String> {
    let escaped = regex::escape(label);
    let pattern = format!(r#"(?s){escaped}</span></div><div[^>]*>(.*?)</div>"#);
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(payload)?;
    let raw = caps.get(1)?.as_str().to_string();
    Some(strip_html(raw))
}

fn strip_html(input: String) -> String {
    Regex::new(r"<[^>]+>")
        .expect("valid html-strip regex")
        .replace_all(&input, "")
        .to_string()
        .trim()
        .to_string()
}
