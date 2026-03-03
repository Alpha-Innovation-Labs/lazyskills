use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lazyskills::config::{AppConfig, SkillsCommandMode};
use lazyskills::services::skills_command::{
    install_skill_from_slug, remove_skill_noninteractive, verify_global_skills_command,
};

struct CwdGuard {
    previous: PathBuf,
}

impl CwdGuard {
    fn switch_to(target: &Path) -> Self {
        let previous = std::env::current_dir().expect("read current directory");
        std::env::set_current_dir(target).expect("switch to test workspace");
        Self { previous }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn temp_workspace() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_millis();
    std::env::temp_dir().join(format!("lazyskills_e2e_{ts}"))
}

#[test]
fn test_install_then_remove_skill_via_service() {
    let workspace = temp_workspace();
    fs::create_dir_all(&workspace).expect("create temporary workspace");
    fs::create_dir_all(workspace.join(".agents/skills")).expect("create project skills directory");
    let _cwd = CwdGuard::switch_to(&workspace);

    let mut cfg = AppConfig::default().skills_command;
    if verify_global_skills_command(&cfg).is_none() {
        cfg.mode = SkillsCommandMode::Npx;
    }

    let slug = "vercel-labs/skills/find-skills";
    let installed_file = workspace
        .join(".agents")
        .join("skills")
        .join("find-skills")
        .join("SKILL.md");

    install_skill_from_slug(&cfg, slug).expect("install skill using CLI service");
    assert!(
        installed_file.exists(),
        "expected installed SKILL.md at {}",
        installed_file.display()
    );

    remove_skill_noninteractive(&cfg, "find-skills").expect("remove skill using CLI service");
    assert!(
        !installed_file.exists(),
        "expected SKILL.md to be removed at {}",
        installed_file.display()
    );

    let _ = fs::remove_dir_all(&workspace);
}
