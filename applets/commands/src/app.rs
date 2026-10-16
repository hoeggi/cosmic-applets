// SPDX-License-Identifier: MPL-2.0

use crate::config::{CommandEntry, Config};
use crate::fl;
use crate::runner::{self, HistoryEntry, RunOutcome};
use cosmic::applet::{menu_button, padded_control};
use cosmic::cosmic_config;
use cosmic::iced::{Alignment, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::{icon_button, section_divider, status_color};
use cosmic_applet_common::{config as cfg, popup};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const APP_ID: &str = "at.hoeggi.CosmicAppletCommands";

/// Number of history entries kept in memory / shown in "Recent runs".
const HISTORY_LEN: usize = 10;
/// Runs longer than this notify even while the popup is open.
const NOTIFY_AFTER: Duration = Duration::from_secs(2);
/// How long the panel icon shows the error state after a failed run.
const ERROR_BADGE_FOR: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
    mode: Mode,
    form: Form,
    runs: HashMap<u64, RunState>,
    expanded: Option<Expanded>,
    history: Vec<HistoryEntry>,
    history_open: bool,
    /// Bumped on every failure so stale `ClearErrorBadge` timers are ignored.
    error_badge: Option<u64>,
    error_generation: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    List,
    Edit,
    /// Add (`None`) or edit an existing command.
    Form(Option<u64>),
}

#[derive(Debug, Default, Clone)]
struct Form {
    name: String,
    command: String,
    in_terminal: bool,
}

impl Form {
    fn is_valid(&self) -> bool {
        !self.name.trim().is_empty() && !self.command.trim().is_empty()
    }
}

#[derive(Debug, Clone)]
enum RunState {
    Running(Instant),
    Done(RunOutcome),
    Terminal { launched: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expanded {
    Command(u64),
    History(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Tick,
    Noop,
    // Running
    Run(u64),
    RunFinished {
        id: u64,
        name: String,
        command: String,
        outcome: RunOutcome,
    },
    TerminalLaunched(u64, bool),
    ToggleOutput(Expanded),
    ToggleHistory,
    ClearErrorBadge(u64),
    // Editing
    EnterEdit,
    ExitEdit,
    AddNew,
    EditEntry(u64),
    Delete(u64),
    Move(u64, isize),
    // Form
    FormName(String),
    FormCommand(String),
    FormTerminal(bool),
    FormSave,
    FormCancel,
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let (config, config_ctx) = cfg::load::<Config>(Self::APP_ID);
        tracing::info!(commands = config.commands.len(), "applet started");

        let app = AppModel {
            core,
            config,
            config_ctx,
            history: runner::load_history(Self::APP_ID, HISTORY_LEN),
            ..Default::default()
        };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The button shown in the panel.
    fn view(&self) -> Element<'_, Self::Message> {
        let icon_name = if self
            .runs
            .values()
            .any(|r| matches!(r, RunState::Running(_)))
        {
            "process-working-symbolic"
        } else if self.error_badge.is_some() {
            "dialog-error-symbolic"
        } else {
            "utilities-terminal-symbolic"
        };
        self.core
            .applet
            .icon_button(icon_name)
            .on_press_down(Message::TogglePopup)
            .into()
    }

    /// The popup.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content = match self.mode {
            Mode::List => self.view_list(),
            Mode::Edit => self.view_edit(),
            Mode::Form(id) => self.view_form(id),
        };
        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs =
            vec![cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig)];
        // Refresh the "running 0:04" timers while the popup is visible.
        if self.popup.is_some()
            && self
                .runs
                .values()
                .any(|r| matches!(r, RunState::Running(_)))
        {
            subs.push(cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                self.mode = Mode::List;
                return popup::toggle(&self.core, &mut self.popup);
            }
            Message::PopupClosed(id) => popup::closed(&mut self.popup, id),
            Message::UpdateConfig(config) => {
                self.runs.retain(|id, _| config.get(*id).is_some());
                self.config = config;
            }
            Message::Tick | Message::Noop => {}

            Message::Run(id) => return self.run(id),
            Message::RunFinished {
                id,
                name,
                command,
                outcome,
            } => return self.run_finished(id, name, command, outcome),
            Message::TerminalLaunched(id, launched) => {
                if !launched {
                    tracing::warn!(id, terminal = ?self.config.terminal, "failed to launch terminal");
                }
                self.runs.insert(id, RunState::Terminal { launched });
            }
            Message::ToggleOutput(target) => {
                self.expanded = (self.expanded != Some(target)).then_some(target);
            }
            Message::ToggleHistory => self.history_open = !self.history_open,
            Message::ClearErrorBadge(generation) => {
                if self.error_badge == Some(generation) {
                    self.error_badge = None;
                }
            }

            Message::EnterEdit => self.mode = Mode::Edit,
            Message::ExitEdit => self.mode = Mode::List,
            Message::AddNew => {
                self.form = Form::default();
                self.mode = Mode::Form(None);
            }
            Message::EditEntry(id) => {
                if let Some(entry) = self.config.get(id) {
                    self.form = Form {
                        name: entry.name.clone(),
                        command: entry.command.clone(),
                        in_terminal: entry.in_terminal,
                    };
                    self.mode = Mode::Form(Some(id));
                }
            }
            Message::Delete(id) => {
                let mut commands = self.config.commands.clone();
                commands.retain(|c| c.id != id);
                self.runs.remove(&id);
                self.save_commands(commands);
            }
            Message::Move(id, delta) => {
                let mut commands = self.config.commands.clone();
                if let Some(from) = commands.iter().position(|c| c.id == id) {
                    let to = from.saturating_add_signed(delta);
                    if to < commands.len() {
                        commands.swap(from, to);
                        self.save_commands(commands);
                    }
                }
            }

            Message::FormName(name) => self.form.name = name,
            Message::FormCommand(command) => self.form.command = command,
            Message::FormTerminal(in_terminal) => self.form.in_terminal = in_terminal,
            Message::FormSave => {
                if let Mode::Form(editing) = self.mode
                    && self.form.is_valid()
                {
                    let entry = CommandEntry {
                        id: editing.unwrap_or_else(|| self.config.next_id()),
                        name: self.form.name.trim().to_owned(),
                        command: self.form.command.trim().to_owned(),
                        in_terminal: self.form.in_terminal,
                    };
                    let mut commands = self.config.commands.clone();
                    match commands.iter_mut().find(|c| c.id == entry.id) {
                        Some(existing) => *existing = entry,
                        None => commands.push(entry),
                    }
                    self.save_commands(commands);
                    self.mode = Mode::Edit;
                }
            }
            Message::FormCancel => self.mode = Mode::Edit,
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    fn run(&mut self, id: u64) -> Task<cosmic::Action<Message>> {
        if matches!(self.runs.get(&id), Some(RunState::Running(_))) {
            return Task::none();
        }
        let Some(entry) = self.config.get(id).cloned() else {
            return Task::none();
        };
        tracing::info!(name = %entry.name, command = %entry.command, terminal = entry.in_terminal, "run");

        if entry.in_terminal {
            let terminal = self.config.terminal.clone();
            return cosmic::task::future(async move {
                Message::TerminalLaunched(
                    id,
                    runner::spawn_in_terminal(terminal, entry.command).await,
                )
            });
        }

        self.runs.insert(id, RunState::Running(Instant::now()));
        cosmic::task::future(async move {
            let outcome = runner::run_captured(entry.command.clone()).await;
            Message::RunFinished {
                id,
                name: entry.name,
                command: entry.command,
                outcome,
            }
        })
    }

    fn run_finished(
        &mut self,
        id: u64,
        name: String,
        command: String,
        outcome: RunOutcome,
    ) -> Task<cosmic::Action<Message>> {
        tracing::info!(
            %name,
            code = ?outcome.code,
            duration_ms = outcome.duration.as_millis(),
            "finished"
        );
        let mut tasks = Vec::new();

        let entry = HistoryEntry::new(name.clone(), command, &outcome);
        self.history.insert(0, entry.clone());
        self.history.truncate(HISTORY_LEN);
        // Indices shift when a new entry is prepended.
        if let Some(Expanded::History(_)) = self.expanded {
            self.expanded = None;
        }
        tasks.push(cosmic::task::future(async move {
            runner::append_history(APP_ID, entry).await;
            Message::Noop
        }));

        if self.popup.is_none() || outcome.duration > NOTIFY_AFTER {
            let (name, outcome) = (name.clone(), outcome.clone());
            tasks.push(cosmic::task::future(async move {
                runner::notify(name, outcome).await;
                Message::Noop
            }));
        }

        if !outcome.success() {
            self.error_generation += 1;
            let generation = self.error_generation;
            self.error_badge = Some(generation);
            tasks.push(cosmic::task::future(async move {
                tokio::time::sleep(ERROR_BADGE_FOR).await;
                Message::ClearErrorBadge(generation)
            }));
        }

        if self.config.get(id).is_some() {
            self.runs.insert(id, RunState::Done(outcome));
        }
        Task::batch(tasks)
    }

    /// Writes the command list through cosmic-config (also updates `self.config`).
    fn save_commands(&mut self, commands: Vec<CommandEntry>) {
        match &self.config_ctx {
            Some(ctx) => {
                if let Err(err) = self.config.set_commands(ctx, commands) {
                    tracing::error!(%err, "failed to save commands");
                }
            }
            None => self.config.commands = commands,
        }
    }

    fn view_list(&self) -> Element<'_, Message> {
        let mut col = widget::Column::new().padding([8, 0]);

        if self.config.commands.is_empty() {
            col = col.push(padded_control(text::body(fl!("no-commands"))));
        }

        for entry in &self.config.commands {
            let (status, glyph) = match self.runs.get(&entry.id) {
                Some(RunState::Running(started)) => {
                    let secs = started.elapsed().as_secs();
                    (
                        fl!("running", time = format!("{}:{:02}", secs / 60, secs % 60)),
                        None,
                    )
                }
                Some(RunState::Done(outcome)) => {
                    let status = match outcome.code {
                        Some(0) => runner::format_duration(outcome.duration),
                        Some(code) => fl!("exit-code", code = code),
                        None => fl!("killed"),
                    };
                    (status, Some(outcome.success()))
                }
                Some(RunState::Terminal { launched: true }) => (fl!("in-terminal"), None),
                Some(RunState::Terminal { launched: false }) => {
                    (fl!("terminal-failed"), Some(false))
                }
                None if entry.in_terminal => (fl!("in-terminal"), None),
                None => (String::new(), None),
            };

            let label = widget::Row::new()
                .push(text::body(&entry.name).width(Length::Fill))
                .push(text::caption(status))
                .spacing(8)
                .align_y(Alignment::Center);

            let mut row = widget::Row::new()
                .push(menu_button(label).on_press(Message::Run(entry.id)))
                .align_y(Alignment::Center);
            let target = Expanded::Command(entry.id);
            if let Some(ok) = glyph {
                let has_output = matches!(self.runs.get(&entry.id), Some(RunState::Done(_)));
                row = row.push(
                    status_button(ok, self.expanded == Some(target))
                        .on_press_maybe(has_output.then_some(Message::ToggleOutput(target))),
                );
            }
            col = col.push(row);

            if self.expanded == Some(target)
                && let Some(RunState::Done(outcome)) = self.runs.get(&entry.id)
            {
                col = col.push(output_block(&outcome.output));
            }
        }

        // Recent runs
        if !self.history.is_empty() {
            col = col.push(section_divider());
            let chevron = if self.history_open { "▾" } else { "▸" };
            col = col.push(
                menu_button(
                    widget::Row::new()
                        .push(
                            text::body(format!("{chevron}  {}", fl!("recent-runs")))
                                .width(Length::Fill),
                        )
                        .push(text::caption(self.history.len().to_string())),
                )
                .on_press(Message::ToggleHistory),
            );
            if self.history_open {
                for (idx, run) in self.history.iter().enumerate() {
                    let target = Expanded::History(idx);
                    let label = widget::Row::new()
                        .push(text::caption(run.time()))
                        .push(text::body(&run.name).width(Length::Fill))
                        .push(text::caption(run.duration()))
                        .push(status_glyph(run.success()))
                        .spacing(12)
                        .align_y(Alignment::Center);
                    col = col.push(menu_button(label).on_press(Message::ToggleOutput(target)));
                    if self.expanded == Some(target) {
                        col = col.push(output_block(&run.output));
                    }
                }
            }
        }

        col = col.push(section_divider());
        col = col.push(
            menu_button(
                widget::Row::new()
                    .push(icon::from_name("document-edit-symbolic").size(16).icon())
                    .push(text::body(fl!("edit-commands")))
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::EnterEdit),
        );
        col.into()
    }

    fn view_edit(&self) -> Element<'_, Message> {
        let mut col = widget::Column::new()
            .padding([8, 0])
            .push(padded_control(text::heading(fl!("edit-commands"))));

        let last = self.config.commands.len().saturating_sub(1);
        for (idx, entry) in self.config.commands.iter().enumerate() {
            let id = entry.id;
            col = col.push(padded_control(
                widget::Row::new()
                    .push(text::body(&entry.name).width(Length::Fill))
                    .push(icon_button("document-edit-symbolic").on_press(Message::EditEntry(id)))
                    .push(
                        icon_button("go-up-symbolic")
                            .on_press_maybe((idx > 0).then_some(Message::Move(id, -1))),
                    )
                    .push(
                        icon_button("go-down-symbolic")
                            .on_press_maybe((idx < last).then_some(Message::Move(id, 1))),
                    )
                    .push(icon_button("edit-delete-symbolic").on_press(Message::Delete(id)))
                    .spacing(4)
                    .align_y(Alignment::Center),
            ));
        }

        col.push(section_divider())
            .push(padded_control(
                widget::Row::new()
                    .push(
                        button::standard(fl!("add-command"))
                            .leading_icon(icon::from_name("list-add-symbolic"))
                            .on_press(Message::AddNew),
                    )
                    .push(widget::space::horizontal())
                    .push(button::suggested(fl!("done")).on_press(Message::ExitEdit)),
            ))
            .into()
    }

    fn view_form(&self, editing: Option<u64>) -> Element<'_, Message> {
        let title = if editing.is_some() {
            fl!("edit-command")
        } else {
            fl!("add-command")
        };
        let spacing = cosmic::theme::spacing();

        let fields = widget::Column::new()
            .push(
                widget::Row::new()
                    .push(icon_button("go-previous-symbolic").on_press(Message::FormCancel))
                    .push(text::heading(title))
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .push(text::caption(fl!("name")))
            .push(
                widget::text_input(fl!("name-placeholder"), &self.form.name)
                    .on_input(Message::FormName),
            )
            .push(text::caption(fl!("command")))
            .push(
                widget::text_input(fl!("command-placeholder"), &self.form.command)
                    .on_input(Message::FormCommand)
                    .on_submit(|_| Message::FormSave),
            )
            .push(text::caption(fl!("command-hint")))
            .push(
                widget::Row::new()
                    .push(text::body(fl!("run-in-terminal")).width(Length::Fill))
                    .push(widget::toggler(self.form.in_terminal).on_toggle(Message::FormTerminal))
                    .align_y(Alignment::Center),
            )
            .spacing(spacing.space_xxs);

        widget::Column::new()
            .padding([8, 0])
            .push(padded_control(fields))
            .push(section_divider())
            .push(padded_control(
                widget::Row::new()
                    .push(widget::space::horizontal())
                    .push(button::standard(fl!("cancel")).on_press(Message::FormCancel))
                    .push(
                        button::suggested(fl!("save"))
                            .on_press_maybe(self.form.is_valid().then_some(Message::FormSave)),
                    )
                    .spacing(spacing.space_xs),
            ))
            .into()
    }
}

fn status_glyph(ok: bool) -> Element<'static, Message> {
    text::body(if ok { "✓" } else { "✗" })
        .class(cosmic::theme::Text::Color(status_color(ok)))
        .into()
}

/// The ✓/✗ next to a command; toggles its output.
fn status_button(ok: bool, expanded: bool) -> button::Button<'static, Message> {
    let chevron = if expanded { "▾" } else { "▸" };
    button::custom(
        widget::Row::new()
            .push(status_glyph(ok))
            .push(text::caption(chevron))
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .class(cosmic::theme::Button::Text)
    .padding([4, 12])
}

fn output_block(output: &str) -> Element<'_, Message> {
    let body = if output.is_empty() {
        "(no output)"
    } else {
        output
    };
    let lines = body.lines().count().min(12) as f32;
    padded_control(
        widget::container(
            widget::scrollable(text::monotext(body).size(11))
                .height(Length::Fixed(lines * 15.0 + 4.0)),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .class(cosmic::theme::Container::Card),
    )
    .into()
}
