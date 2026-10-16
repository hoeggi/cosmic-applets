// SPDX-License-Identifier: MPL-2.0

use crate::config::Config;
use crate::fl;
use crate::keyring;
use crate::model::{self, Event, Filter, Panel, Seen, State};
use crate::ring;
use crate::rpc::{Client, RpcError, Snapshot, Torrent};
use cosmic::applet::{menu_button, padded_control};
use cosmic::cosmic_config;
use cosmic::iced::advanced::text::EllipsizeHeightLimit;
use cosmic::iced::widget::stack;
use cosmic::iced::widget::text::{Ellipsize, LineHeight, Wrapping};
use cosmic::iced::{Alignment, Color, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::{error_text, icon_button, section_divider, status_color};
use cosmic_applet_common::{config as cfg, notify, popup};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub const APP_ID: &str = "at.hoeggi.CosmicAppletTransmission";

/// Poll interval while the popup is open.
const POPUP_POLL: Duration = Duration::from_secs(3);
const LIST_MAX_HEIGHT: f32 = 420.0;

#[derive(Debug, Default)]
enum Status {
    #[default]
    NotConfigured,
    LoadingPassword,
    Connecting,
    Connected,
    Failed(RpcError),
    Keyring(String),
}

#[derive(Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    List,
    Settings,
}

/// The settings page while it is being edited.
#[derive(Debug, Clone, Default)]
struct Form {
    host: String,
    port: String,
    rpc_path: String,
    username: String,
    password: String,
    show_password: bool,
    client_command: String,
    notify_finished: bool,
    notify_errors: bool,
}

impl Form {
    fn new(config: &Config, password: &str) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port.to_string(),
            rpc_path: config.rpc_path.clone(),
            username: config.username.clone(),
            password: password.to_owned(),
            show_password: false,
            client_command: config.client_command.join(" "),
            notify_finished: config.notify_finished,
            notify_errors: config.notify_errors,
        }
    }

    fn port(&self) -> Option<u16> {
        self.port.trim().parse().ok().filter(|p| *p > 0)
    }

    fn is_valid(&self) -> bool {
        !self.host.trim().is_empty() && self.port().is_some()
    }
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    page: Page,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
    /// `None` until the password is known and while not configured.
    client: Option<Arc<Client>>,
    /// Bumped on every reconnect; results of older requests are dropped.
    generation: u64,
    polling: bool,
    /// Password from the keyring; `Some("")` when none is stored.
    password: Option<String>,
    status: Status,
    /// Last successful poll.
    snapshot: Option<Snapshot>,
    /// Torrent states from the last successful poll, for notifications.
    seen: Option<HashMap<i64, Seen>>,
    filter: Filter,
    form: Form,
    /// Problem saving the password, shown on the list page.
    notice: Option<String>,
    panel_icon: Option<(Panel, icon::Handle)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Poll,
    Polled(u64, Result<Snapshot, RpcError>),
    PasswordLoaded(u64, Result<Option<String>, String>),
    PasswordStored(Result<(), String>),
    Retry,
    SetFilter(Filter),
    OpenClient,
    OpenSettings,
    CloseSettings,
    FormHost(String),
    FormPort(String),
    FormRpcPath(String),
    FormUsername(String),
    FormPassword(String),
    FormShowPassword,
    FormClientCommand(String),
    FormNotifyFinished(bool),
    FormNotifyErrors(bool),
    SaveSettings,
    Noop,
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
        tracing::info!(server = %config.server(), "applet started");
        let mut app = AppModel {
            core,
            config,
            config_ctx,
            ..Default::default()
        };
        let task = app.connect();
        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The button shown in the panel: the ring with the count inside.
    fn view(&self) -> Element<'_, Self::Message> {
        let (panel, handle) = self.panel_icon.clone().unwrap_or_else(|| {
            let panel = self.panel();
            (panel, ring::handle(panel))
        });
        let size = f32::from(self.core.applet.suggested_size(true).0);
        let (label, dim) = ring::label(panel);

        let ring = widget::icon(handle)
            .class(cosmic::theme::Svg::Custom(Rc::new(|theme| {
                cosmic::iced::widget::svg::Style {
                    color: Some(on_panel(theme, 1.0)),
                }
            })))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size));
        let label = text(label)
            .size((size * 0.5).round().max(8.0))
            .font(cosmic::font::bold())
            .line_height(LineHeight::Relative(1.0))
            .class(cosmic::theme::Text::Color(on_panel(
                &cosmic::theme::active(),
                if dim { 0.55 } else { 1.0 },
            )));

        self.core
            .applet
            .button_from_element(
                stack![ring, widget::container(label).center(Length::Fixed(size))],
                true,
            )
            .on_press_down(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content = match self.page {
            Page::List => self.view_list(),
            Page::Settings => self.view_settings(),
        };
        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs =
            vec![cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig)];
        if self.client.is_some() {
            let interval = if self.popup.is_some() {
                POPUP_POLL
            } else {
                Duration::from_secs(self.config.poll_secs.max(5))
            };
            subs.push(cosmic::iced::time::every(interval).map(|_| Message::Poll));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                let task = popup::toggle(&self.core, &mut self.popup);
                if self.popup.is_none() {
                    return task;
                }
                self.page = Page::List;
                return Task::batch([task, self.poll()]);
            }
            Message::PopupClosed(id) => popup::closed(&mut self.popup, id),
            Message::UpdateConfig(config) => {
                let reconnect = config.connection() != self.config.connection();
                self.config = config;
                if reconnect {
                    // Different server or user: their password may differ too.
                    self.password = None;
                    return self.connect();
                }
            }
            Message::Poll => return self.poll(),
            Message::Polled(generation, result) => {
                if generation == self.generation {
                    return self.polled(result);
                }
            }
            Message::PasswordLoaded(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                match result {
                    Ok(password) => {
                        if password.is_none() {
                            tracing::info!("no password stored in the keyring");
                        }
                        self.password = Some(password.unwrap_or_default());
                        return self.connect();
                    }
                    Err(err) => {
                        tracing::warn!(%err, "cannot read password from keyring");
                        self.status = Status::Keyring(err);
                        self.update_panel_icon();
                    }
                }
            }
            Message::PasswordStored(result) => {
                self.notice = result.err().map(|err| {
                    tracing::warn!(%err, "cannot store password in keyring");
                    fl!("keyring-store-error", error = err)
                });
            }
            Message::Retry => {
                if matches!(self.status, Status::Keyring(_)) {
                    self.password = None;
                }
                return self.connect();
            }
            Message::SetFilter(filter) => self.filter = filter,
            Message::OpenClient => return self.open_client(),
            Message::OpenSettings => {
                self.form = Form::new(&self.config, self.password.as_deref().unwrap_or(""));
                self.page = Page::Settings;
            }
            Message::CloseSettings => self.page = Page::List,
            Message::FormHost(v) => self.form.host = v,
            Message::FormPort(v) => self.form.port = v,
            Message::FormRpcPath(v) => self.form.rpc_path = v,
            Message::FormUsername(v) => self.form.username = v,
            Message::FormPassword(v) => self.form.password = v,
            Message::FormShowPassword => self.form.show_password = !self.form.show_password,
            Message::FormClientCommand(v) => self.form.client_command = v,
            Message::FormNotifyFinished(on) => self.form.notify_finished = on,
            Message::FormNotifyErrors(on) => self.form.notify_errors = on,
            Message::SaveSettings => return self.save_settings(),
            Message::Noop => {}
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    /// (Re)starts the connection from the current config: fetches the password
    /// from the keyring first when a username is set and it is not known yet.
    fn connect(&mut self) -> Task<cosmic::Action<Message>> {
        self.generation += 1;
        self.polling = false;
        self.client = None;
        self.snapshot = None;
        self.seen = None;

        let task = if !self.config.is_configured() {
            self.status = Status::NotConfigured;
            Task::none()
        } else if !self.config.username.is_empty() && self.password.is_none() {
            self.status = Status::LoadingPassword;
            let generation = self.generation;
            let server = self.config.server();
            let username = self.config.username.clone();
            cosmic::task::future(async move {
                Message::PasswordLoaded(generation, keyring::load(server, username).await)
            })
        } else {
            self.status = Status::Connecting;
            self.client = Some(Arc::new(Client::new(
                self.config.url(),
                &self.config.username,
                self.password.clone(),
            )));
            self.poll()
        };
        self.update_panel_icon();
        task
    }

    fn poll(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        if self.polling {
            return Task::none();
        }
        self.polling = true;
        let generation = self.generation;
        cosmic::task::future(async move { Message::Polled(generation, client.snapshot().await) })
    }

    fn polled(&mut self, result: Result<Snapshot, RpcError>) -> Task<cosmic::Action<Message>> {
        self.polling = false;
        let mut task = Task::none();
        match result {
            Ok(snapshot) => {
                if !matches!(self.status, Status::Connected) {
                    tracing::info!(torrents = snapshot.torrents.len(), "connected");
                }
                if let Some(seen) = &self.seen {
                    task = self.notify(model::events(seen, &snapshot));
                }
                self.seen = Some(model::seen(&snapshot));
                self.snapshot = Some(snapshot);
                self.status = Status::Connected;
            }
            Err(err) => {
                if !matches!(&self.status, Status::Failed(prev) if *prev == err) {
                    tracing::warn!(%err, "poll failed");
                }
                // Keep `seen`, so downloads that finish during an outage still notify.
                self.status = Status::Failed(err);
            }
        }
        self.update_panel_icon();
        task
    }

    fn notify(&self, events: Vec<Event>) -> Task<cosmic::Action<Message>> {
        let notifications: Vec<_> = events
            .into_iter()
            .filter_map(|event| match event {
                Event::Finished(name) if self.config.notify_finished => {
                    Some(("emblem-ok-symbolic", fl!("finished"), name))
                }
                Event::Failed { name, error } if self.config.notify_errors => Some((
                    "dialog-error-symbolic",
                    fl!("failed"),
                    format!("{name}\n{error}"),
                )),
                _ => None,
            })
            .collect();
        if notifications.is_empty() {
            return Task::none();
        }
        cosmic::task::future(async move {
            for (icon, summary, body) in notifications {
                notify::send("Transmission", icon, summary, body).await;
            }
            Message::Noop
        })
    }

    fn open_client(&mut self) -> Task<cosmic::Action<Message>> {
        let mut tasks = Vec::new();
        if self.popup.is_some() {
            tasks.push(popup::toggle(&self.core, &mut self.popup));
        }
        let command = self.config.client_command.clone();
        tasks.push(cosmic::task::future(async move {
            launch(command).await;
            Message::Noop
        }));
        Task::batch(tasks)
    }

    fn save_settings(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(port) = self.form.port() else {
            return Task::none();
        };
        let form = self.form.clone();
        let before = self.config.connection();
        let old_password = self.password.clone().unwrap_or_default();

        let host = form.host.trim().to_owned();
        let rpc_path = form.rpc_path.trim().to_owned();
        let username = form.username.trim().to_owned();
        let command: Vec<String> = form
            .client_command
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        self.save(|c, ctx| c.set_host(ctx, host));
        self.save(|c, ctx| c.set_port(ctx, port));
        self.save(|c, ctx| c.set_rpc_path(ctx, rpc_path));
        self.save(|c, ctx| c.set_username(ctx, username));
        self.save(|c, ctx| c.set_client_command(ctx, command));
        self.save(|c, ctx| c.set_notify_finished(ctx, form.notify_finished));
        self.save(|c, ctx| c.set_notify_errors(ctx, form.notify_errors));
        self.page = Page::List;
        self.notice = None;

        let changed = self.config.connection() != before || form.password != old_password;
        let mut tasks = Vec::new();
        if changed {
            if !self.config.username.is_empty() && !form.password.is_empty() {
                let (server, username) = (self.config.server(), self.config.username.clone());
                let password = form.password.clone();
                tasks.push(cosmic::task::future(async move {
                    Message::PasswordStored(keyring::store(server, username, password).await)
                }));
            }
            self.password = Some(form.password);
            tasks.push(self.connect());
        }
        Task::batch(tasks)
    }

    /// Writes one config field through cosmic-config (also updates `self.config`).
    fn save(
        &mut self,
        set: impl FnOnce(&mut Config, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
    ) {
        cfg::save(&mut self.config, self.config_ctx.as_ref(), set);
    }

    fn panel(&self) -> Panel {
        match (&self.status, &self.snapshot) {
            (Status::Connected, Some(snapshot)) => Panel::from_snapshot(snapshot),
            _ => Panel::Offline,
        }
    }

    /// Rebuilds the ring only when what it shows changes (avoids re-rasterising).
    fn update_panel_icon(&mut self) {
        let panel = self.panel();
        if self.panel_icon.as_ref().map(|(p, _)| *p) != Some(panel) {
            self.panel_icon = Some((panel, ring::handle(panel)));
        }
    }

    fn view_list(&self) -> Element<'_, Message> {
        let mut col = widget::Column::new()
            .padding([8, 0])
            .push(padded_control(self.view_header()));
        if let Some(notice) = &self.notice {
            col = col.push(padded_control(error_text(notice.clone())));
        }

        col = match (&self.status, &self.snapshot) {
            (Status::NotConfigured, _) => col.push(padded_control(
                widget::Column::new()
                    .push(text::body(fl!("not-configured")))
                    .push(button::suggested(fl!("set-up")).on_press(Message::OpenSettings))
                    .spacing(8),
            )),
            (Status::Failed(err), _) => col.push(self.view_problem(self.describe(err))),
            (Status::Keyring(err), _) => {
                col.push(self.view_problem(fl!("keyring-error", error = err.clone())))
            }
            (_, Some(snapshot)) => col
                .push(padded_control(self.view_tabs(snapshot)))
                .push(self.view_torrents(snapshot)),
            _ => col.push(padded_control(text::body(fl!("connecting")))),
        };

        col.push(section_divider())
            .push(
                menu_button(
                    widget::Row::new()
                        .push(text::body(fl!("open-client")).width(Length::Fill))
                        .push(icon::from_name("go-next-symbolic").size(16).icon())
                        .align_y(Alignment::Center),
                )
                .on_press(Message::OpenClient),
            )
            .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let title = if self.config.is_configured() {
            self.config.host.trim().to_owned()
        } else {
            fl!("app-title")
        };
        let mut info = widget::Column::new().push(text::heading(title));
        if let (Status::Connected, Some(snapshot)) = (&self.status, &self.snapshot) {
            let down = snapshot.torrents.iter().map(|t| t.rate_download).sum();
            let up = snapshot.torrents.iter().map(|t| t.rate_upload).sum();
            info = info.push(text::caption(fl!(
                "rates",
                down = model::rate(down),
                up = model::rate(up)
            )));
        }
        let mut row = widget::Row::new()
            .push(info.width(Length::Fill))
            .spacing(8)
            .align_y(Alignment::Center);
        if self.snapshot.as_ref().is_some_and(|s| s.alt_speed) {
            row = row.push(text::caption(fl!("turtle")).class(cosmic::theme::Text::Accent));
        }
        row.push(icon_button("emblem-system-symbolic").on_press(Message::OpenSettings))
            .into()
    }

    fn view_problem(&self, message: String) -> Element<'_, Message> {
        padded_control(
            widget::Column::new()
                .push(error_text(message))
                .push(
                    widget::Row::new()
                        .push(button::standard(fl!("retry")).on_press(Message::Retry))
                        .push(button::standard(fl!("settings")).on_press(Message::OpenSettings))
                        .spacing(8),
                )
                .spacing(8),
        )
        .into()
    }

    fn describe(&self, err: &RpcError) -> String {
        match err {
            RpcError::Unreachable(_) => fl!("unreachable", server = self.config.server()),
            RpcError::Unauthorized => fl!("unauthorized"),
            RpcError::Http(code) => fl!("http-error", server = self.config.server(), code = code),
            RpcError::Server(msg) => fl!("server-error", error = msg.clone()),
            RpcError::Protocol(msg) => fl!("protocol-error", error = msg.clone()),
        }
    }

    fn view_tabs(&self, snapshot: &Snapshot) -> Element<'_, Message> {
        let mut row = widget::Row::new().spacing(4);
        for filter in Filter::ALL {
            let label = format!(
                "{} {}",
                filter_label(filter),
                model::count(snapshot, filter)
            );
            let msg = Message::SetFilter(filter);
            row = row.push(if filter == self.filter {
                button::suggested(label).on_press(msg)
            } else {
                button::text(label).on_press(msg)
            });
        }
        widget::scrollable::horizontal(row).into()
    }

    fn view_torrents<'a>(&self, snapshot: &'a Snapshot) -> Element<'a, Message> {
        let list = model::visible(snapshot, self.filter);
        if list.is_empty() {
            return padded_control(text::body(fl!("no-torrents"))).into();
        }
        let col = list
            .into_iter()
            .fold(widget::Column::new(), |col, t| col.push(torrent_row(t)));
        widget::container(widget::scrollable(col).height(Length::Shrink))
            .max_height(LIST_MAX_HEIGHT)
            .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let form = &self.form;
        let spacing = cosmic::theme::spacing();
        let field = |label: String, input: Element<'static, Message>| {
            widget::Column::new()
                .push(text::caption(label))
                .push(input)
                .spacing(spacing.space_xxxs)
        };

        let fields = widget::Column::new()
            .push(
                widget::Row::new()
                    .push(icon_button("go-previous-symbolic").on_press(Message::CloseSettings))
                    .push(text::heading(fl!("settings")))
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .push(field(
                fl!("host"),
                widget::text_input("192.168.0.10", form.host.clone())
                    .on_input(Message::FormHost)
                    .into(),
            ))
            .push(
                widget::Row::new()
                    .push(
                        field(
                            fl!("port"),
                            widget::text_input("9091", form.port.clone())
                                .on_input(Message::FormPort)
                                .into(),
                        )
                        .width(Length::FillPortion(1)),
                    )
                    .push(
                        field(
                            fl!("rpc-path"),
                            widget::text_input("/transmission/rpc", form.rpc_path.clone())
                                .on_input(Message::FormRpcPath)
                                .into(),
                        )
                        .width(Length::FillPortion(2)),
                    )
                    .spacing(spacing.space_xs),
            )
            .push(field(
                fl!("username"),
                widget::text_input(fl!("username-hint"), form.username.clone())
                    .on_input(Message::FormUsername)
                    .into(),
            ))
            .push(field(
                fl!("password"),
                widget::secure_input(
                    "",
                    form.password.clone(),
                    Some(Message::FormShowPassword),
                    !form.show_password,
                )
                .on_input(Message::FormPassword)
                .on_submit(|_| Message::SaveSettings)
                .into(),
            ))
            .push(text::caption(fl!("password-hint")))
            .push(field(
                fl!("client-command"),
                widget::text_input("", form.client_command.clone())
                    .on_input(Message::FormClientCommand)
                    .into(),
            ))
            .push(text::caption(fl!("client-command-hint")))
            .push(toggle_row(
                fl!("notify-finished"),
                form.notify_finished,
                Message::FormNotifyFinished,
            ))
            .push(toggle_row(
                fl!("notify-errors"),
                form.notify_errors,
                Message::FormNotifyErrors,
            ))
            .spacing(spacing.space_xs);

        widget::Column::new()
            .padding([8, 0])
            .push(padded_control(fields))
            .push(section_divider())
            .push(padded_control(
                widget::Row::new()
                    .push(widget::space::horizontal())
                    .push(button::standard(fl!("cancel")).on_press(Message::CloseSettings))
                    .push(
                        button::suggested(fl!("save"))
                            .on_press_maybe(form.is_valid().then_some(Message::SaveSettings)),
                    )
                    .spacing(spacing.space_xs),
            ))
            .into()
    }
}

/// One list row: name, progress bar and a status line. Clicking opens the client.
fn torrent_row(t: &Torrent) -> Element<'_, Message> {
    let state = t.state();
    let percent = format!("{:.0} %", t.percent_done * 100.0);
    let left: Vec<String> = match state {
        State::Downloading => vec![percent, format!("↓ {}", model::rate(t.rate_download))],
        State::Seeding => vec![format!("↑ {}", model::rate(t.rate_upload))],
        State::Paused => vec![percent, fl!("paused")],
        State::Checking => vec![percent, fl!("checking")],
        State::Queued => vec![percent, fl!("queued")],
        State::Error => vec![percent],
    };
    let right = match state {
        State::Downloading => model::eta(t.eta).unwrap_or_default(),
        State::Seeding => fl!("ratio", ratio = model::ratio(t.upload_ratio)),
        _ => String::new(),
    };
    let meta = left
        .into_iter()
        .fold(widget::Row::new().spacing(12), |row, s| {
            row.push(text::caption(s))
        })
        .push(widget::space::horizontal())
        .push(text::caption(right));

    let mut bar = widget::determinate_linear(t.percent_done as f32)
        .width(Length::Fill)
        .girth(4.0);
    bar = match state {
        State::Downloading => bar,
        State::Seeding => bar.bar_color(status_color(true)),
        State::Error => bar.bar_color(status_color(false)),
        _ => bar.bar_color(muted()),
    };

    let mut col = widget::Column::new()
        .push(
            text::body(&t.name)
                .wrapping(Wrapping::None)
                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
        )
        .push(bar)
        .push(meta)
        .spacing(4);
    if !t.error_string.is_empty() {
        col = col.push(if state == State::Error {
            error_text(t.error_string.clone())
        } else {
            text::caption(t.error_string.clone()).into()
        });
    }
    menu_button(col).on_press(Message::OpenClient).into()
}

fn filter_label(filter: Filter) -> String {
    match filter {
        Filter::Active => fl!("filter-active"),
        Filter::Downloading => fl!("filter-downloading"),
        Filter::Seeding => fl!("filter-seeding"),
        Filter::All => fl!("filter-all"),
    }
}

fn toggle_row(label: String, on: bool, msg: fn(bool) -> Message) -> Element<'static, Message> {
    widget::Row::new()
        .push(text::body(label).width(Length::Fill))
        .push(widget::toggler(on).on_toggle(msg))
        .align_y(Alignment::Center)
        .into()
}

/// The panel's foreground colour, as used for symbolic applet icons.
fn on_panel(theme: &cosmic::Theme, alpha: f32) -> Color {
    let mut color: Color = theme.cosmic().background(theme.transparent).on.into();
    color.a *= alpha;
    color
}

fn muted() -> Color {
    let mut color: Color = cosmic::theme::active().cosmic().on_bg_color().into();
    color.a = 0.35;
    color
}

async fn launch(command: Vec<String>) {
    let Some((program, args)) = command.split_first() else {
        tracing::warn!("no client command configured");
        return;
    };
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    if cosmic::process::spawn(cmd).await.is_none() {
        tracing::warn!(?command, "failed to start client");
    }
}
