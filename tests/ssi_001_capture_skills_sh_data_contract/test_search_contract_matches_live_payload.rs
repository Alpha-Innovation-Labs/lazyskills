use skills_tui::adapters::skills_sh::SkillsShClient;

#[test]
fn test_search_contract_matches_live_payload() {
    let client = SkillsShClient::new().expect("client should initialize");
    let response = client
        .fetch_search("find-skills", 10)
        .expect("search response should be fetched");

    assert!(response.count > 0, "count should be greater than zero");
    assert!(
        !response.skills.is_empty(),
        "skills array should include at least one entry"
    );
    assert!(response.skills.len() <= 10);

    let first = &response.skills[0];
    let has_slug = first.id.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
    let has_source_skill = !first.source.is_empty() && first.skill_id.is_some();
    assert!(
        has_slug || has_source_skill,
        "either id or source+skill_id must be present"
    );
    assert!(!first.name.is_empty(), "name should be present");
    assert!(first.installs > 0, "installs should be positive");
}
