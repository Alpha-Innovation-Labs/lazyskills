use skills_tui::adapters::skills_sh::{SkillsMode, SkillsShClient};

#[test]
fn test_list_contract_matches_live_payload() {
    let client = SkillsShClient::new().expect("client should initialize");
    let page = client
        .fetch_catalog_page(SkillsMode::AllTime, 1)
        .expect("catalog page should be fetched");

    assert_eq!(page.page, 1);
    assert!(page.total > 0, "total should be greater than zero");
    assert!(!page.skills.is_empty(), "skills array should be non-empty");
    assert!(page.skills.len() <= 200, "page size should stay <= 200");

    let sample = &page.skills[0];
    assert!(sample.skill_id.is_some(), "skill_id should be present");
    assert!(!sample.source.is_empty(), "source should be present");
    assert!(!sample.name.is_empty(), "name should be present");
    assert!(sample.installs > 0, "installs should be a positive number");
}
