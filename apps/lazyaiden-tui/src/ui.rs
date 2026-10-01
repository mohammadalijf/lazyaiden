//! Rendering. Reads `App`, draws widgets; holds no state of its own.

use lazyaiden_core::ProfileDraft;
use lazyaiden_core::profiles::SyncState;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Clear, List as ListWidget, ListItem, ListState, Paragraph, Wrap};
use serde_json::Value;

use crate::forms::{Kind, ProfileForm, Prompt, ScheduleForm};
use crate::model::{DeviceRow, Origin, Panel, ProfileRow};
use crate::state::{App, Confirm, Mode};

const FOCUS: Color = Color::Green;
const DIM: Color = Color::DarkGray;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner(app: &App) -> &'static str {
    SPINNER[app.spinner % SPINNER.len()]
}

/// A dim placeholder inside a panel: a spinner while the first refresh runs, `empty` after.
fn placeholder(frame: &mut Frame, app: &App, area: Rect, what: &str, empty: &str) {
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let text = if app.loaded {
        empty.to_owned()
    } else {
        format!("{} loading {what}…", spinner(app))
    };
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(DIM)), inner);
}

/// Draws the whole interface.
pub fn draw(frame: &mut Frame, app: &App) {
    let [main, status] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).areas(frame.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)]).areas(main);
    let [profiles, schedules, device] = Layout::vertical([
        Constraint::Percentage(48),
        Constraint::Percentage(30),
        Constraint::Min(4),
    ])
    .areas(left);

    draw_profiles(frame, app, profiles);
    draw_schedules(frame, app, schedules);
    draw_devices(frame, app, device);
    draw_preview(frame, app, right);
    draw_status(frame, app, status);

    match &app.mode {
        Mode::Normal => {}
        Mode::Help => draw_help(frame),
        Mode::Profile(f) => draw_profile_form(frame, f),
        Mode::Schedule(f) => draw_schedule_form(frame, f),
        Mode::Prompt(p) => draw_prompt(frame, p),
        Mode::Confirm(c) => draw_confirm(frame, c),
        Mode::Brewer(list) => draw_picker(frame, &list.items, list.selected),
    }
}

/// Renders the interface into plain text; used by snapshot tests.
pub fn render_text(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
    terminal.draw(|f| draw(f, app)).expect("draw");
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn block(title: &str, focused: bool) -> Block<'static> {
    let style = Style::default().fg(if focused { FOCUS } else { DIM });
    Block::bordered().border_style(style).title(Span::styled(
        format!(" {title} "),
        if focused {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        },
    ))
}

fn list_state(selected: usize, len: usize) -> ListState {
    ListState::default().with_selected((len > 0).then_some(selected))
}

fn highlight(focused: bool) -> Style {
    if focused {
        Style::default().bg(FOCUS).fg(Color::Black)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

fn badge(state: Option<SyncState>) -> Span<'static> {
    let (sym, color) = match state {
        Some(SyncState::InSync) => ("✓", Color::Green),
        Some(SyncState::Modified) => ("~", Color::Yellow),
        Some(SyncState::LocalOnly) => ("+", Color::Blue),
        Some(SyncState::RemoteMissing) => ("!", Color::Red),
        Some(SyncState::RemoteOnly) => ("↓", Color::Magenta),
        None => ("·", DIM),
    };
    Span::styled(sym, Style::default().fg(color))
}

fn state_text(state: Option<SyncState>) -> &'static str {
    match state {
        Some(SyncState::InSync) => "in sync with the brewer",
        Some(SyncState::Modified) => "modified: local differs from the brewer",
        Some(SyncState::LocalOnly) => "local only: not on the brewer yet",
        Some(SyncState::RemoteMissing) => "linked, but missing on the brewer",
        Some(SyncState::RemoteOnly) => "on the brewer only: pull it to edit",
        None => "brewer state unknown",
    }
}

fn origin_tag(origin: Option<Origin>) -> Span<'static> {
    match origin {
        Some(Origin::Fellow) => Span::styled("[F] ", Style::default().fg(Color::Cyan)),
        Some(Origin::Custom) => Span::styled("[C] ", Style::default().fg(Color::Yellow)),
        None => Span::raw("    "),
    }
}

fn origin_text(origin: Option<Origin>) -> Option<&'static str> {
    match origin {
        Some(Origin::Fellow) => Some("made by Fellow"),
        Some(Origin::Custom) => Some("created by you"),
        None => None,
    }
}

fn draw_profiles(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Panel::Profiles;
    let items: Vec<ListItem> = app
        .profiles
        .items
        .iter()
        .map(|r| {
            ListItem::new(Line::from(vec![
                badge(r.state),
                Span::raw(" "),
                origin_tag(r.origin()),
                Span::raw(r.title.clone()),
            ]))
        })
        .collect();
    let empty = items.is_empty();
    let hidden = app.hidden_fellow();
    let title = if hidden > 0 {
        format!("1 Profiles ({hidden} Fellow hidden, f shows)")
    } else {
        "1 Profiles".to_owned()
    };
    let mut state = list_state(app.profiles.selected, items.len());
    let list = ListWidget::new(items)
        .block(block(&title, focused))
        .highlight_style(highlight(focused));
    frame.render_stateful_widget(list, area, &mut state);
    if empty {
        placeholder(
            frame,
            app,
            area,
            "profiles",
            "No profiles yet: n new, i import, P pull",
        );
    }
}

fn draw_schedules(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Panel::Schedules;
    let items: Vec<ListItem> = app
        .schedules
        .items
        .iter()
        .map(|v| {
            let d = &v.schedule.draft;
            let time = v.time().map_or_else(|| "??:??".into(), |t| t.to_string());
            let on = if d.enabled {
                Span::styled("●", Style::default().fg(Color::Green))
            } else {
                Span::styled("○", Style::default().fg(DIM))
            };
            ListItem::new(Line::from(vec![
                on,
                Span::raw(format!(" {time} {} ", v.days())),
                Span::styled(
                    v.profile_title
                        .clone()
                        .unwrap_or_else(|| "(missing profile)".into()),
                    Style::default().fg(DIM),
                ),
            ]))
        })
        .collect();
    let empty = items.is_empty();
    let mut state = list_state(app.schedules.selected, items.len());
    let list = ListWidget::new(items)
        .block(block("2 Schedules", focused))
        .highlight_style(highlight(focused));
    frame.render_stateful_widget(list, area, &mut state);
    if empty {
        placeholder(frame, app, area, "schedules", "");
    }
}

fn device_label(r: &DeviceRow) -> String {
    r.device.label()
}

fn draw_devices(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Panel::Device;
    let items: Vec<ListItem> = app
        .devices
        .items
        .iter()
        .map(|r| {
            ListItem::new(format!(
                "{} {}",
                if r.active { "▸" } else { " " },
                device_label(r)
            ))
        })
        .collect();
    let empty = items.is_empty();
    let mut state = list_state(app.devices.selected, items.len());
    let list = ListWidget::new(items)
        .block(block("3 Device", focused))
        .highlight_style(highlight(focused));
    frame.render_stateful_widget(list, area, &mut state);
    if empty {
        placeholder(frame, app, area, "brewers", "");
    }
}

fn num(v: f64) -> String {
    format!("{v}")
}

fn draft_lines(d: &ProfileDraft) -> Vec<Line<'static>> {
    let temps = |t: &[f64]| t.iter().map(|x| num(*x)).collect::<Vec<_>>().join(", ");
    let on = |b: bool| if b { "on" } else { "off" };
    let row = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("{k:<14}"), Style::default().fg(DIM)),
            Span::raw(v),
        ])
    };
    vec![
        row("Ratio", format!("1:{}", num(d.ratio))),
        row(
            "Bloom",
            format!(
                "{} · {}× · {}s · {}°C",
                on(d.bloom_enabled),
                num(d.bloom_ratio),
                d.bloom_duration,
                num(d.bloom_temperature)
            ),
        ),
        row(
            "Single-serve",
            format!(
                "{} · {} pulses · every {}s · {}",
                on(d.ss_pulses_enabled),
                d.ss_pulses_number,
                d.ss_pulses_interval,
                temps(&d.ss_pulse_temperatures)
            ),
        ),
        row(
            "Batch",
            format!(
                "{} · {} pulses · every {}s · {}",
                on(d.batch_pulses_enabled),
                d.batch_pulses_number,
                d.batch_pulses_interval,
                temps(&d.batch_pulse_temperatures)
            ),
        ),
    ]
}

fn diff_lines(local: &ProfileDraft, remote: &ProfileDraft) -> Vec<Line<'static>> {
    let (Ok(Value::Object(l)), Ok(Value::Object(r))) =
        (serde_json::to_value(local), serde_json::to_value(remote))
    else {
        return Vec::new();
    };
    l.iter()
        .filter(|(k, v)| r.get(*k) != Some(v))
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{k}: "), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{v}"), Style::default().fg(Color::Green)),
                Span::raw(" (local)  "),
                Span::styled(format!("{}", r[k]), Style::default().fg(Color::Red)),
                Span::raw(" (brewer)"),
            ])
        })
        .collect()
}

fn profile_preview(row: &ProfileRow, app: &App) -> Text<'static> {
    let mut lines = vec![
        Line::from(Span::styled(
            row.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            state_text(row.state),
            Style::default().fg(DIM),
        )),
        Line::default(),
    ];
    if let Some(l) = &row.local {
        lines.push(Line::from(format!("file    {}", l.name)));
    }
    if let Some(id) = &row.remote_id {
        let mut line = Line::from(format!("remote  {id}"));
        if let Some(o) = origin_text(row.origin()) {
            line.push_span(Span::styled(format!("  ({o})"), Style::default().fg(DIM)));
        }
        lines.push(line);
    }
    lines.push(Line::default());
    let draft = row
        .local
        .as_ref()
        .map(|l| l.draft.clone())
        .or_else(|| row.remote.as_ref().and_then(|r| r.draft().ok()));
    if let Some(d) = &draft {
        lines.extend(draft_lines(d));
        let issues = d.issues();
        if !issues.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                "Not pushable yet:",
                Style::default().fg(Color::Red),
            )));
            lines.extend(issues.iter().map(|i| {
                Line::from(Span::styled(
                    format!("  {i}"),
                    Style::default().fg(Color::Red),
                ))
            }));
        }
    }
    if let (Some(l), Some(r)) = (&row.local, row.remote.as_ref().and_then(|r| r.draft().ok())) {
        let diff = diff_lines(&l.draft, &r);
        if !diff.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                "Differences",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.extend(diff);
        }
    }
    if !app.problems.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Unreadable files",
            Style::default().fg(Color::Red),
        )));
        lines.extend(
            app.problems
                .iter()
                .map(|p| Line::from(format!("  {}: {}", p.name, p.message))),
        );
    }
    Text::from(lines)
}

fn draw_preview(frame: &mut Frame, app: &App, area: Rect) {
    if !app.loaded {
        frame.render_widget(
            Paragraph::new(format!("{} loading from the brewer…", spinner(app)))
                .style(Style::default().fg(DIM))
                .block(block("Details", false)),
            area,
        );
        return;
    }
    let (title, text) = match app.focus {
        Panel::Profiles => (
            "Details",
            app.selected_profile().map_or_else(
                || Text::from(vec![Line::from("Nothing selected.")]),
                |r| profile_preview(r, app),
            ),
        ),
        Panel::Schedules => (
            "Schedule",
            app.schedules.current().map_or_else(
                || Text::from("Nothing selected."),
                |v| {
                    let d = &v.schedule.draft;
                    Text::from(vec![
                        Line::from(format!("id        {}", v.schedule.id)),
                        Line::from(format!(
                            "enabled   {}",
                            if d.enabled { "yes" } else { "no" }
                        )),
                        Line::from(format!("days      {}", v.days())),
                        Line::from(format!(
                            "time      {}",
                            v.time().map_or_else(
                                || d.second_from_start_of_the_day.to_string(),
                                |t| t.to_string()
                            )
                        )),
                        Line::from(format!("water     {} ml", d.amount_of_water)),
                        Line::from(format!(
                            "profile   {} ({})",
                            v.profile_title.clone().unwrap_or_else(|| "missing".into()),
                            d.profile_id
                        )),
                    ])
                },
            ),
        ),
        Panel::Device => (
            "Brewer",
            app.devices.current().map_or_else(
                || Text::from("No brewers loaded."),
                |r| {
                    let mut lines = vec![
                        Line::from(format!("id      {}", r.device.id)),
                        Line::from(format!(
                            "name    {}",
                            r.device.display_name.clone().unwrap_or_else(|| "-".into())
                        )),
                        Line::from(format!(
                            "active  {}",
                            if r.active { "yes" } else { "no (Enter to use)" }
                        )),
                    ];
                    for (k, v) in &r.device.extra {
                        if !v.is_object() && !v.is_array() {
                            lines.push(Line::from(format!("{k:<8}{v}")));
                        }
                    }
                    Text::from(lines)
                },
            ),
        ),
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(block(title, false))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn hints(app: &App) -> &'static str {
    match app.mode {
        Mode::Normal => match app.focus {
            Panel::Profiles => {
                "n new  e edit  p push  P pull  d delete  s share  f Fellow  ? help  q quit"
            }
            Panel::Schedules => "n new  t toggle  d delete  r refresh  Tab panel  ? help  q quit",
            Panel::Device => "Enter choose brewer  r refresh  Tab panel  ? help  q quit",
        },
        Mode::Help => "any key closes help",
        Mode::Profile(_) | Mode::Schedule(_) => {
            "Tab/↓ next  ↑ prev  Space toggle  Ctrl-S save  Esc cancel"
        }
        Mode::Prompt(_) => "Enter confirm  Esc cancel",
        Mode::Confirm(_) => "y yes  n no",
        Mode::Brewer(_) => "↑/↓ choose  Enter select  Esc cancel",
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let active = app
        .devices
        .items
        .iter()
        .find(|d| d.active)
        .map(device_label);
    let mut right = Vec::new();
    if app.busy > 0 {
        let doing = app.activity.as_deref().unwrap_or("working");
        right.push(Span::styled(
            format!("{} {doing}… ", spinner(app)),
            Style::default().fg(Color::Yellow),
        ));
    }
    let tail = if !app.loaded {
        None
    } else if !app.remote_ok {
        Some("offline".to_owned())
    } else {
        active
    };
    if let Some(t) = tail {
        right.push(Span::styled(t, Style::default().fg(DIM)));
    }
    let right_text = Line::from(right);
    let right_width = (right_text.width() as u16 + 1).min(area.width / 2);
    let [left, right] =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(right_width)]).areas(area);
    let line = match &app.message {
        Some(m) => Line::from(Span::styled(
            m.text.clone(),
            Style::default().fg(if m.error { Color::Red } else { Color::Green }),
        )),
        None => Line::from(Span::styled(hints(app), Style::default().fg(DIM))),
    };
    frame.render_widget(Paragraph::new(line), left);
    frame.render_widget(Paragraph::new(right_text).right_aligned(), right);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn popup(frame: &mut Frame, title: &str, width: u16, height: u16) -> Rect {
    let area = centered(frame.area(), width, height);
    frame.render_widget(Clear, area);
    let b = Block::bordered()
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {title} "));
    let inner = b.inner(area);
    frame.render_widget(b, area);
    inner
}

fn draw_help(frame: &mut Frame) {
    let rows = [
        ("Tab / 1 2 3", "switch panel"),
        ("j k ↑ ↓  g G", "move / first / last"),
        ("r", "refresh from the brewer"),
        ("b", "choose brewer"),
        ("", ""),
        ("Profiles", ""),
        ("n", "new profile (template)"),
        ("e / Enter", "edit in a form"),
        ("E", "edit the YAML in $EDITOR"),
        ("i / x", "import / export a YAML file"),
        ("p / P", "push to / pull from the brewer"),
        ("U / D", "push all / pull all"),
        ("d", "delete"),
        ("s / L", "copy share link / import link"),
        ("f", "show / hide Fellow profiles"),
        ("[F] [C]", "made by Fellow / created by you"),
        ("", ""),
        ("Schedules", ""),
        ("n  t  d", "new / toggle / delete"),
        ("", ""),
        ("? q Ctrl-C", "help / quit"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                Line::from(Span::styled(
                    *k,
                    Style::default().add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("{k:<16}"), Style::default().fg(Color::Yellow)),
                    Span::raw(*v),
                ])
            }
        })
        .collect();
    let inner = popup(frame, "Keys", 60, lines.len() as u16 + 2);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn field_line(label: &str, value: &str, focused: bool, error: Option<&str>) -> Line<'static> {
    let label_style = if focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(DIM)
    };
    let mut spans = vec![
        Span::styled(format!("{label:<26}"), label_style),
        Span::styled(
            format!("{value}{}", if focused { "▏" } else { "" }),
            if focused {
                Style::default().add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
            },
        ),
    ];
    if let Some(e) = error {
        spans.push(Span::styled(
            format!("  ✗ {e}"),
            Style::default().fg(Color::Red),
        ));
    }
    Line::from(spans)
}

fn draw_profile_form(frame: &mut Frame, f: &ProfileForm) {
    let title = f
        .name
        .as_deref()
        .map_or_else(|| "New profile".to_owned(), |n| format!("Edit {n}"));
    let height = f.fields.len() as u16 + 4;
    let inner = popup(frame, &title, 96, height);
    let mut lines: Vec<Line> = f
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let value = if field.kind == Kind::Bool {
                if field.value == "true" {
                    "[x] on".to_owned()
                } else {
                    "[ ] off".to_owned()
                }
            } else {
                field.value.clone()
            };
            let focused = i == f.focus;
            let err = f.issue_for(field.key).map(|i| i.message.as_str());
            let mut line = field_line(
                field.label,
                &value,
                focused && field.kind != Kind::Bool,
                err,
            );
            if focused && field.kind == Kind::Bool {
                line.spans[0].style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);
            }
            line
        })
        .collect();
    lines.push(Line::default());
    lines.push(if f.issues.is_empty() {
        Line::from(Span::styled(
            "valid — Ctrl-S to save",
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(Span::styled(
            format!("{} problem(s) — fix them to save", f.issues.len()),
            Style::default().fg(Color::Red),
        ))
    });
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_schedule_form(frame: &mut Frame, f: &ScheduleForm) {
    let inner = popup(frame, "New schedule", 80, ScheduleForm::ROWS as u16 + 5);
    let profile = f
        .profiles
        .get(f.profile)
        .map_or_else(|| "(none)".into(), |(id, t)| format!("‹ {t} ({id}) ›"));
    let mut lines = vec![
        field_line(
            "Profile (←/→)",
            &profile,
            false,
            f.issue_for("profile").map(|i| i.message.as_str()),
        ),
        field_line(
            "Days (mon-fri, sat,sun)",
            &f.days,
            f.focus == 1,
            f.issue_for("days").map(|i| i.message.as_str()),
        ),
        field_line(
            "Time (07:30 / 7:30am)",
            &f.time,
            f.focus == 2,
            f.issue_for("time").map(|i| i.message.as_str()),
        ),
        field_line(
            "Water (ml, 150–1500)",
            &f.water,
            f.focus == 3,
            f.issue_for("water").map(|i| i.message.as_str()),
        ),
        field_line(
            "Enabled",
            if f.enabled { "[x] on" } else { "[ ] off" },
            false,
            None,
        ),
    ];
    // Mark the focused row for the non-text rows too.
    let focused = f.focus;
    if matches!(focused, 0 | 4) {
        lines[focused].spans[0].style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
    }
    lines.push(Line::default());
    lines.push(if f.issues.is_empty() {
        Line::from(Span::styled(
            "valid — Ctrl-S to create",
            Style::default().fg(Color::Green),
        ))
    } else {
        Line::from(Span::styled(
            format!("{} problem(s)", f.issues.len()),
            Style::default().fg(Color::Red),
        ))
    });
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_prompt(frame: &mut Frame, p: &Prompt) {
    let inner = popup(frame, &p.label, 70, 3);
    frame.render_widget(Paragraph::new(format!("{}▏", p.value)), inner);
}

fn draw_confirm(frame: &mut Frame, c: &Confirm) {
    let width = (c.text.chars().count() as u16 + 4).clamp(30, 100);
    let inner = popup(frame, "Confirm", width, 3);
    frame.render_widget(
        Paragraph::new(c.text.clone()).wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_picker(frame: &mut Frame, items: &[DeviceRow], selected: usize) {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|r| {
            ListItem::new(format!(
                "{} {}",
                if r.active { "▸" } else { " " },
                device_label(r)
            ))
        })
        .collect();
    let inner = popup(frame, "Choose a brewer", 50, items.len() as u16 + 2);
    let mut state = list_state(selected, items.len());
    frame.render_stateful_widget(
        ListWidget::new(list_items).highlight_style(highlight(true)),
        inner,
        &mut state,
    );
}
