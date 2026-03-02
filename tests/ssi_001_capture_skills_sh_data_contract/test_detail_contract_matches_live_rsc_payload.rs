use skills_tui::adapters::skills_sh::{DetailSource, SkillsShClient};

#[test]
fn test_detail_contract_matches_live_rsc_payload() {
    let client = SkillsShClient::new().expect("client should initialize");
    let detail = client
        .fetch_skill_detail("vercel-labs", "skills", "find-skills")
        .expect("skill detail should be fetched");

    assert_eq!(detail.source, DetailSource::Rsc, "should parse from RSC");
    assert!(!detail.weekly_installs.is_empty());
    assert!(!detail.repository.is_empty());
    assert!(!detail.github_stars.is_empty());
    assert!(!detail.first_seen.is_empty());
    assert!(
        !detail.installed_on.is_empty(),
        "installed-on breakdown should be present"
    );
}
