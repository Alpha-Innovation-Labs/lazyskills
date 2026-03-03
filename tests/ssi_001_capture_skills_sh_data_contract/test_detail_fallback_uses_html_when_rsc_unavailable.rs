use lazyskills::adapters::skills_sh::{DetailSource, SkillsShClient};

#[test]
fn test_detail_fallback_uses_html_when_rsc_unavailable() {
    let client = SkillsShClient::new().expect("client should initialize");
    let detail = client
        .fetch_skill_detail_with_rsc_url(
            "vercel-labs",
            "skills",
            "find-skills",
            "https://skills.sh/this-route-does-not-exist.rsc",
        )
        .expect("HTML fallback detail should be fetched");

    assert_eq!(
        detail.source,
        DetailSource::HtmlFallback,
        "HTML fallback should be used when RSC is unavailable"
    );
    assert_eq!(detail.repository, "vercel-labs/skills");
    assert!(!detail.weekly_installs.is_empty());
}
