use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use ratkit::primitives::pane::Pane;
use skills_tui::adapters::skills_sh::SkillDetail;

const CONSOLE_BG: Color = Color::Rgb(10, 14, 20);
const CONSOLE_TEXT: Color = Color::Rgb(217, 225, 234);
const CONSOLE_MUTED: Color = Color::Rgb(122, 134, 148);
const CONSOLE_ACCENT: Color = Color::Rgb(53, 194, 255);
const CONSOLE_SUCCESS: Color = Color::Rgb(95, 211, 141);
const CONSOLE_WARNING: Color = Color::Rgb(246, 193, 119);
const CONSOLE_ERROR: Color = Color::Rgb(247, 118, 142);

pub struct SkillDetailPaneData<'a> {
    pub detail: Option<&'a SkillDetail>,
    pub empty_line_1: &'a str,
    pub empty_line_2: &'a str,
}

fn sanitize_metric(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(pos) = trimmed.rfind('>') {
        trimmed[pos + 1..].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn section_title(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(CONSOLE_MUTED),
    ))
}

fn status_color(status: &str) -> Color {
    match status.to_ascii_uppercase().as_str() {
        "PASS" => CONSOLE_SUCCESS,
        "WARN" => CONSOLE_WARNING,
        "FAIL" => CONSOLE_ERROR,
        _ => CONSOLE_TEXT,
    }
}

pub fn render_skill_detail_pane(
    frame: &mut Frame,
    area: Rect,
    terminal_icon: &str,
    border_color: Color,
    data: SkillDetailPaneData<'_>,
) {
    let detail_pane = Pane::new("Details")
        .with_icon(terminal_icon)
        .border_style(Style::default().fg(border_color));
    let (detail_inner, _) = detail_pane.render_block(frame, area);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(CONSOLE_BG)),
        detail_inner,
    );

    let mut detail_lines = Vec::new();
    if let Some(detail) = data.detail {
        detail_lines.push(section_title("WEEKLY INSTALLS"));
        detail_lines.push(Line::from(Span::styled(
            sanitize_metric(&detail.weekly_installs),
            Style::default().fg(CONSOLE_TEXT),
        )));
        detail_lines.push(Line::from(""));

        detail_lines.push(section_title("REPOSITORY"));
        detail_lines.push(Line::from(Span::styled(
            format!("https://github.com/{}", detail.repository),
            Style::default().fg(CONSOLE_TEXT),
        )));
        detail_lines.push(Line::from(""));

        detail_lines.push(section_title("GITHUB STARS"));
        detail_lines.push(Line::from(vec![
            Span::styled("☆ ", Style::default().fg(CONSOLE_ACCENT)),
            Span::styled(
                sanitize_metric(&detail.github_stars),
                Style::default().fg(CONSOLE_TEXT),
            ),
        ]));
        detail_lines.push(Line::from(""));

        detail_lines.push(section_title("FIRST SEEN"));
        detail_lines.push(Line::from(Span::styled(
            sanitize_metric(&detail.first_seen),
            Style::default().fg(CONSOLE_TEXT),
        )));
        detail_lines.push(Line::from(""));

        if !detail.security_audits.is_empty() {
            detail_lines.push(section_title("SECURITY AUDITS"));
            let max_audit_width = detail
                .security_audits
                .iter()
                .map(|audit| audit.name.chars().count())
                .max()
                .unwrap_or(0)
                .min(28);
            for audit in &detail.security_audits {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<width$}", audit.name, width = max_audit_width),
                        Style::default().fg(CONSOLE_TEXT),
                    ),
                    Span::styled("  ", Style::default().fg(CONSOLE_MUTED)),
                    Span::styled(
                        format!("{:<4}", audit.status),
                        Style::default().fg(status_color(&audit.status)),
                    ),
                ]));
            }
            detail_lines.push(Line::from(""));
        }

        if !detail.installed_on.is_empty() {
            detail_lines.push(section_title("INSTALLED ON"));
            let max_agent_width = detail
                .installed_on
                .iter()
                .map(|entry| entry.agent.chars().count())
                .max()
                .unwrap_or(0)
                .min(22);
            for install in &detail.installed_on {
                detail_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<width$}", install.agent, width = max_agent_width),
                        Style::default().fg(CONSOLE_TEXT),
                    ),
                    Span::styled("  ", Style::default().fg(CONSOLE_MUTED)),
                    Span::styled(install.installs.clone(), Style::default().fg(CONSOLE_MUTED)),
                ]));
            }
        }
    } else {
        detail_lines.push(Line::from(data.empty_line_1));
        detail_lines.push(Line::from(data.empty_line_2));
    }

    frame.render_widget(
        Paragraph::new(detail_lines)
            .style(Style::default().fg(CONSOLE_TEXT).bg(CONSOLE_BG))
            .wrap(Wrap { trim: true }),
        detail_inner,
    );
}
