use lazyskills::adapters::skills_sh::{SkillsMode, SkillsShClient};

#[test]
fn test_end_to_end_catalog_and_detail_ingestion() {
    let client = SkillsShClient::new().expect("client should initialize");

    let page = client
        .fetch_catalog_page(SkillsMode::AllTime, 1)
        .expect("catalog page should be fetched");
    let skill = page
        .skills
        .iter()
        .find(|item| !item.source.is_empty() && item.skill_id.is_some())
        .expect("catalog should contain at least one valid source + skill_id entry");

    let mut source_parts = skill.source.split('/');
    let owner = source_parts.next().expect("owner segment should exist");
    let repo = source_parts.next().expect("repo segment should exist");
    let skill_name = skill
        .skill_id
        .as_deref()
        .expect("skill_id should exist for selected catalog record");

    let expected_slug = format!("{owner}/{repo}/{skill_name}");

    let detail = client
        .fetch_skill_detail(owner, repo, skill_name)
        .expect("detail fetch should succeed from catalog slug");

    assert_eq!(
        detail.slug, expected_slug,
        "detail slug should match catalog-derived slug"
    );
    assert!(!detail.weekly_installs.is_empty());
    assert!(!detail.repository.is_empty());
}
