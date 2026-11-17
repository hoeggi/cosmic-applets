// SPDX-License-Identifier: MPL-2.0

use crate::api::{Air, AirCurrent, ApiError, Client, Forecast};
use crate::chart;
use crate::config::{self, Config, Location, REFRESH_CHOICES, Units};
use crate::fl;
use crate::model::{self, Alert, Day, Pollen, Timeline, When};
use cosmic::applet::{menu_button, padded_control};
use cosmic::cosmic_config;
use cosmic::iced::widget::{container as iced_container, responsive, stack};
use cosmic::iced::{Alignment, Background, Border, Color, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::{error_text, icon_button, section_divider};
use cosmic_applet_common::{config as cfg, popup};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const APP_ID: &str = "at.hoeggi.CosmicAppletWeather";

/// Wakes up to check whether a fetch is due and to update the "updated … ago" line.
const TICK: Duration = Duration::from_secs(60);
/// After a failed fetch, try again this soon (or at the refresh interval, if shorter).
const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// After HTTP 429, wait this long.
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(60 * 60);
/// Data older than this is marked stale (dimmed in the panel, warning in the popup).
const STALE_AFTER_MINUTES: i64 = 60;
const POPUP_MAX_HEIGHT: f32 = 680.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    Weather,
    Settings,
    Search,
}

/// The last successful fetch.
#[derive(Debug, Clone)]
struct Data {
    forecast: Forecast,
    /// `None` when air and pollen are both turned off.
    air: Option<Result<Air, ApiError>>,
    fetched: chrono::DateTime<chrono::Local>,
    units: Units,
}

type FetchResult = Result<(Forecast, Option<Result<Air, ApiError>>), ApiError>;

#[derive(Debug, Default)]
struct Search {
    query: String,
    /// The query the results are for.
    searched: String,
    results: Option<Result<Vec<Location>, ApiError>>,
    searching: bool,
    latitude: String,
    longitude: String,
}

impl Search {
    fn coordinates(&self) -> Option<(f64, f64)> {
        Some((
            config::parse_coordinate(&self.latitude, 90.0)?,
            config::parse_coordinate(&self.longitude, 180.0)?,
        ))
    }
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    page: Page,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
    client: Client,
    /// Bumped whenever what is fetched changes; older results are dropped.
    generation: u64,
    fetching: bool,
    data: Option<Data>,
    /// The last fetch failed; shown with the old data if there is any.
    error: Option<ApiError>,
    next_fetch: Option<Instant>,
    /// The day opened in the 7-day list.
    expanded: Option<chrono::NaiveDate>,
    search: Search,
    /// Language for place names in search results, e.g. `de`.
    language: String,
    refresh_labels: Vec<String>,
    /// The chart rendered for a width and data; regenerated only when either changes.
    chart: RefCell<Option<(ChartKey, widget::svg::Handle)>>,
}

#[derive(Debug, Clone, PartialEq)]
struct ChartKey {
    width: u32,
    timeline: Timeline,
    colors: chart::Colors,
    units: Units,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Tick,
    Refresh,
    Fetched(u64, Box<FetchResult>),
    ToggleDay(chrono::NaiveDate),
    OpenPage(Page),
    SelectLocation(usize),
    DeleteLocation(usize),
    SetUnits(Units),
    SetPanelTemperature(bool),
    SetPanelPrecipitation(bool),
    SetPanelHighLow(bool),
    SetShowAir(bool),
    SetShowPollen(bool),
    SetRefresh(usize),
    SearchQuery(String),
    Search,
    Searched(String, Result<Vec<Location>, ApiError>),
    AddLocation(Location),
    Latitude(String),
    Longitude(String),
    AddCoordinates,
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
        let language = i18n_embed::DesktopLanguageRequester::requested_languages()
            .first()
            .map_or_else(|| "en".to_owned(), |l| l.language.as_str().to_owned());
        tracing::info!(locations = config.locations.len(), "applet started");
        let mut app = AppModel {
            core,
            config,
            config_ctx,
            language,
            refresh_labels: REFRESH_CHOICES
                .iter()
                .map(|m| fl!("minutes", minutes = m))
                .collect(),
            ..Default::default()
        };
        let task = app.fetch();
        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.view_panel()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content = match self.page {
            Page::Weather => self.view_weather(),
            Page::Settings => self.view_settings(),
            Page::Search => self.view_search(),
        };
        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch([
            cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig),
            cosmic::iced::time::every(TICK).map(|_| Message::Tick),
        ])
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                let task = popup::toggle(&self.core, &mut self.popup);
                if self.popup.is_some() {
                    self.page = Page::Weather;
                }
                return task;
            }
            Message::PopupClosed(id) => popup::closed(&mut self.popup, id),
            Message::UpdateConfig(config) => {
                let before = self.config.request();
                self.config = config;
                return self.config_changed(before);
            }
            Message::Tick => {
                if self.next_fetch.is_none_or(|at| Instant::now() >= at) {
                    return self.fetch();
                }
            }
            Message::Refresh => return self.fetch(),
            Message::Fetched(generation, result) => {
                if generation == self.generation {
                    self.fetched(*result);
                }
            }
            Message::ToggleDay(date) => {
                self.expanded = (self.expanded != Some(date)).then_some(date);
            }
            Message::OpenPage(page) => {
                if page == Page::Search {
                    self.search = Search::default();
                }
                self.page = page;
            }
            Message::SelectLocation(index) => {
                return self.set(|c, ctx| c.set_active(ctx, index));
            }
            Message::DeleteLocation(index) => {
                let mut locations = self.config.locations.clone();
                if index >= locations.len() {
                    return Task::none();
                }
                locations.remove(index);
                let active = match self.config.active {
                    a if a > index => a - 1,
                    a if a == index => 0,
                    a => a,
                };
                let before = self.config.request();
                self.save(|c, ctx| c.set_locations(ctx, locations));
                self.save(|c, ctx| c.set_active(ctx, active));
                return self.config_changed(before);
            }
            Message::SetUnits(units) => return self.set(|c, ctx| c.set_units(ctx, units)),
            Message::SetPanelTemperature(on) => {
                return self.set(|c, ctx| c.set_panel_temperature(ctx, on));
            }
            Message::SetPanelPrecipitation(on) => {
                return self.set(|c, ctx| c.set_panel_precipitation(ctx, on));
            }
            Message::SetPanelHighLow(on) => {
                return self.set(|c, ctx| c.set_panel_high_low(ctx, on));
            }
            Message::SetShowAir(on) => return self.set(|c, ctx| c.set_show_air(ctx, on)),
            Message::SetShowPollen(on) => return self.set(|c, ctx| c.set_show_pollen(ctx, on)),
            Message::SetRefresh(index) => {
                if let Some(mins) = REFRESH_CHOICES.get(index).copied() {
                    self.save(|c, ctx| c.set_refresh_mins(ctx, mins));
                    // Count the new interval from the last fetch.
                    if let Some(data) = &self.data {
                        let age = (chrono::Local::now() - data.fetched)
                            .to_std()
                            .unwrap_or_default();
                        self.next_fetch = Some(
                            Instant::now() + Duration::from_secs(mins * 60).saturating_sub(age),
                        );
                    }
                }
            }
            Message::SearchQuery(query) => self.search.query = query,
            Message::Search => {
                let query = self.search.query.trim().to_owned();
                if query.is_empty() || self.search.searching {
                    return Task::none();
                }
                self.search.searching = true;
                let (client, language) = (self.client.clone(), self.language.clone());
                return cosmic::task::future(async move {
                    let result = client.search(&query, &language).await;
                    Message::Searched(query, result)
                });
            }
            Message::Searched(query, result) => {
                if let Err(err) = &result {
                    tracing::warn!(%err, "place search failed");
                }
                self.search.searching = false;
                self.search.searched = query;
                self.search.results = Some(result);
            }
            Message::AddLocation(location) => return self.add_location(location),
            Message::Latitude(v) => self.search.latitude = v,
            Message::Longitude(v) => self.search.longitude = v,
            Message::AddCoordinates => {
                if let Some((lat, lon)) = self.search.coordinates() {
                    return self.add_location(Location::from_coordinates(lat, lon));
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

// ---------------------------------------------------------------- state

impl AppModel {
    fn fetch(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(location) = self.config.location().cloned() else {
            self.next_fetch = None;
            return Task::none();
        };
        if self.fetching {
            return Task::none();
        }
        self.fetching = true;
        // Until this one finishes; `fetched` sets the next one.
        self.next_fetch = Some(Instant::now() + RETRY_AFTER);
        let (client, units, air) = (
            self.client.clone(),
            self.config.units,
            self.config.wants_air(),
        );
        let generation = self.generation;
        cosmic::task::future(async move {
            let result = match client.forecast(&location, units).await {
                Ok(forecast) => {
                    let air = if air {
                        Some(client.air(&location).await)
                    } else {
                        None
                    };
                    Ok((forecast, air))
                }
                Err(err) => Err(err),
            };
            Message::Fetched(generation, Box::new(result))
        })
    }

    fn fetched(&mut self, result: FetchResult) {
        self.fetching = false;
        let refresh = Duration::from_secs(self.config.refresh_mins() * 60);
        match result {
            Ok((forecast, air)) => {
                if let Some(Err(err)) = &air {
                    tracing::warn!(%err, "air quality fetch failed");
                }
                if self.error.take().is_some() || self.data.is_none() {
                    tracing::info!("forecast received");
                }
                self.data = Some(Data {
                    forecast,
                    air,
                    fetched: chrono::Local::now(),
                    units: self.config.units,
                });
                self.next_fetch = Some(Instant::now() + refresh);
            }
            Err(err) => {
                if self.error.as_ref() != Some(&err) {
                    tracing::warn!(%err, "forecast fetch failed");
                }
                let wait = if err == ApiError::RateLimited {
                    RATE_LIMIT_BACKOFF
                } else {
                    RETRY_AFTER.min(refresh)
                };
                self.next_fetch = Some(Instant::now() + wait);
                self.error = Some(err);
            }
        }
    }

    /// Reacts to a config change: a different place or units make the shown data wrong,
    /// turning on air or pollen needs a fetch.
    fn config_changed(
        &mut self,
        before: (Option<Location>, Units, bool),
    ) -> Task<cosmic::Action<Message>> {
        let now = self.config.request();
        if now == before {
            return Task::none();
        }
        if (&now.0, now.1) != (&before.0, before.1) {
            self.data = None;
            self.error = None;
            self.expanded = None;
        }
        self.generation += 1;
        self.fetching = false;
        self.fetch()
    }

    fn add_location(&mut self, location: Location) -> Task<cosmic::Action<Message>> {
        let before = self.config.request();
        let mut locations = self.config.locations.clone();
        let index = locations
            .iter()
            .position(|l| l.latitude == location.latitude && l.longitude == location.longitude)
            .unwrap_or_else(|| {
                locations.push(location);
                locations.len() - 1
            });
        self.save(|c, ctx| c.set_locations(ctx, locations));
        self.save(|c, ctx| c.set_active(ctx, index));
        // The first place leads straight to its weather; later ones back to the list.
        self.page = if before.0.is_none() {
            Page::Weather
        } else {
            Page::Settings
        };
        self.config_changed(before)
    }

    /// Writes one config field and refetches if that changes what is fetched.
    fn set(
        &mut self,
        set: impl FnOnce(&mut Config, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
    ) -> Task<cosmic::Action<Message>> {
        let before = self.config.request();
        self.save(set);
        self.config_changed(before)
    }

    fn save(
        &mut self,
        set: impl FnOnce(&mut Config, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
    ) {
        cfg::save(&mut self.config, self.config_ctx.as_ref(), set);
    }

    fn age_minutes(&self) -> Option<i64> {
        self.data
            .as_ref()
            .map(|d| (chrono::Local::now() - d.fetched).num_minutes().max(0))
    }

    fn is_stale(&self) -> bool {
        self.age_minutes().is_some_and(|m| m >= STALE_AFTER_MINUTES)
    }

    fn alert(&self) -> Option<Alert> {
        let data = self.data.as_ref()?;
        let now = model::local_now(data.forecast.utc_offset_seconds);
        model::alert(&data.forecast.minutely_15, now, data.units)
    }

    fn days(&self) -> Vec<Day> {
        self.data.as_ref().map_or_else(Vec::new, |d| {
            model::days(&d.forecast, model::local_now(d.forecast.utc_offset_seconds))
        })
    }
}

// ---------------------------------------------------------------- panel

impl AppModel {
    /// Icon and temperature; optionally the next rain or snow and today's high / low.
    fn view_panel(&self) -> Element<'_, Message> {
        let horizontal = self.core.applet.is_horizontal();
        let size = f32::from(self.core.applet.suggested_size(true).0);
        let dim = self.data.is_none() || self.is_stale();
        let alpha = if dim { 0.5 } else { 1.0 };

        let current = self.data.as_ref().map(|d| &d.forecast.current);
        let name = match (self.config.location(), current) {
            (None, _) => "find-location-symbolic",
            (_, Some(c)) => model::icon(c.weather_code.unwrap_or(0), c.is_day != Some(0)),
            _ => "weather-few-clouds-symbolic",
        };
        let mut items: Vec<Element<'_, Message>> = vec![panel_icon(name, size, alpha)];

        if self.config.panel_temperature
            && let Some(temp) = current.and_then(|c| c.temperature_2m)
        {
            items.push(self.panel_text(model::temperature(temp), alpha));
        }
        if self.config.panel_precipitation
            && let Some(alert) = self.alert()
        {
            let mut row = widget::Row::new()
                .push(panel_icon(alert.kind.icon(), (size * 0.7).round(), alpha))
                .spacing(2)
                .align_y(Alignment::Center);
            if let When::Starts(at) = alert.when {
                row = row.push(self.panel_text(model::clock(at.time()), alpha * 0.8));
            }
            items.push(row.into());
        }
        if self.config.panel_high_low
            && let Some(day) = self.days().first()
            && let (Some(lo), Some(hi)) = (day.min, day.max)
        {
            items.push(self.panel_text(
                format!("{} / {}", model::temperature(hi), model::temperature(lo)),
                alpha * 0.8,
            ));
        }

        let content: Element<'_, Message> = if horizontal {
            items
                .into_iter()
                .fold(widget::Row::new().spacing(6), |row, item| row.push(item))
                .align_y(Alignment::Center)
                .into()
        } else {
            items
                .into_iter()
                .fold(widget::Column::new().spacing(2), |col, item| col.push(item))
                .align_x(Alignment::Center)
                .into()
        };
        let (major, minor) = self.core.applet.suggested_padding(true);
        let padding = if horizontal {
            [minor, major]
        } else {
            [major, minor]
        };
        button::custom(content)
            .padding(padding)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::TogglePopup)
            .into()
    }

    fn panel_text(&self, label: String, alpha: f32) -> Element<'_, Message> {
        self.core
            .applet
            .text(label)
            .class(cosmic::theme::Text::Color(on_panel(
                &cosmic::theme::active(),
                alpha,
            )))
            .into()
    }
}

/// A symbolic icon in the panel's foreground colour.
fn panel_icon(name: &'static str, size: f32, alpha: f32) -> Element<'static, Message> {
    widget::icon(icon::from_name(name).symbolic(true).handle())
        .class(cosmic::theme::Svg::Custom(Rc::new(move |theme| {
            cosmic::iced::widget::svg::Style {
                color: Some(on_panel(theme, alpha)),
            }
        })))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

/// The panel's foreground colour, as used for symbolic applet icons.
fn on_panel(theme: &cosmic::Theme, alpha: f32) -> Color {
    let mut color: Color = theme.cosmic().background(theme.transparent).on.into();
    color.a *= alpha;
    color
}

// ---------------------------------------------------------------- popup: weather

impl AppModel {
    fn view_weather(&self) -> Element<'_, Message> {
        let mut col = widget::Column::new()
            .padding([8, 0])
            .push(padded_control(self.view_header()))
            .push(section_divider());

        let Some(_) = self.config.location() else {
            return col.push(self.view_first_run()).into();
        };
        let Some(data) = &self.data else {
            let body: Element<'_, Message> = match &self.error {
                Some(err) => widget::Column::new()
                    .push(error_text(describe(err)))
                    .push(button::standard(fl!("retry")).on_press(Message::Refresh))
                    .spacing(8)
                    .into(),
                None => text::body(fl!("loading")).into(),
            };
            return col.push(padded_control(body)).into();
        };

        if let Some(err) = &self.error {
            col = col.push(padded_control(self.view_banner(err, data)));
        }
        let now = model::local_now(data.forecast.utc_offset_seconds);
        let mut body = widget::Column::new().push(padded_control(self.view_now(data, now)));
        if let Some(alert) = model::alert(&data.forecast.minutely_15, now, data.units) {
            body = body.push(padded_control(view_alert(&alert, data.units)));
        }
        if let Some(timeline) = model::timeline(&data.forecast, now) {
            body = body
                .push(section_divider())
                .push(padded_control(group_heading(fl!("next-24-hours"))))
                .push(padded_control(self.view_timeline(timeline, data.units)));
        }
        if self.config.show_air || self.config.show_pollen {
            body = body.push(section_divider());
            match &data.air {
                Some(Ok(air)) => {
                    let uv = model::days(&data.forecast, now)
                        .first()
                        .and_then(|d| d.uv_max);
                    if self.config.show_air {
                        body = body.push(padded_control(view_air(&air.current, uv)));
                    }
                    if self.config.show_pollen {
                        body = body.push(padded_control(view_pollen(&air.current)));
                    }
                }
                Some(Err(err)) => {
                    body = body.push(padded_control(error_text(fl!(
                        "air-error",
                        error = describe(err)
                    ))));
                }
                // Turned on since the last fetch, which is under way.
                None => body = body.push(padded_control(text::caption(fl!("loading")))),
            }
        }
        body = body
            .push(section_divider())
            .push(self.view_days(now, data.units))
            .push(padded_control(
                widget::container(text::caption(fl!("attribution"))).center_x(Length::Fill),
            ));

        col.push(
            widget::container(widget::scrollable(body).height(Length::Shrink))
                .max_height(POPUP_MAX_HEIGHT),
        )
        .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let title = self
            .config
            .location()
            .map_or_else(|| fl!("app-title"), |l| l.name.clone());
        let mut info = widget::Column::new().push(text::heading(title));
        if let Some(minutes) = self.age_minutes() {
            info = info.push(if minutes >= STALE_AFTER_MINUTES {
                widget::Row::new()
                    .push(icon::from_name("dialog-warning-symbolic").size(12).icon())
                    .push(
                        text::caption(fl!("updated-hours", hours = minutes.div_euclid(60)))
                            .class(warning_text()),
                    )
                    .spacing(4)
                    .align_y(Alignment::Center)
                    .into()
            } else {
                Element::from(text::caption(fl!("updated", minutes = minutes)))
            });
        }
        let mut row = widget::Row::new()
            .push(info.width(Length::Fill))
            .spacing(4)
            .align_y(Alignment::Center);
        if self.config.location().is_some() {
            row = row.push(
                icon_button("view-refresh-symbolic")
                    .on_press_maybe((!self.fetching).then_some(Message::Refresh)),
            );
        }
        row.push(icon_button("emblem-system-symbolic").on_press(Message::OpenPage(Page::Settings)))
            .into()
    }

    fn view_first_run(&self) -> Element<'_, Message> {
        padded_control(
            widget::Column::new()
                .push(icon::from_name("find-location-symbolic").size(32).icon())
                .push(text::heading(fl!("no-location")))
                .push(text::caption(fl!("no-location-hint")).align_x(Alignment::Center))
                .push(
                    button::suggested(fl!("choose-location"))
                        .on_press(Message::OpenPage(Page::Search)),
                )
                .spacing(8)
                .padding([16, 8])
                .align_x(Alignment::Center)
                .width(Length::Fill),
        )
        .into()
    }

    fn view_banner<'a>(&self, err: &ApiError, data: &Data) -> Element<'a, Message> {
        widget::Column::new()
            .push(error_text(describe(err)))
            .push(text::caption(fl!(
                "showing-old",
                time = data.fetched.format("%H:%M").to_string()
            )))
            .spacing(2)
            .into()
    }

    fn view_now<'a>(&self, data: &Data, now: chrono::NaiveDateTime) -> Element<'a, Message> {
        let current = &data.forecast.current;
        let code = current.weather_code.unwrap_or(0);
        let mut info = widget::Column::new()
            .push(text::title2(
                current
                    .temperature_2m
                    .map_or_else(|| "–".to_owned(), model::temperature_precise),
            ))
            .push(text::body(fl!("condition", code = code)));
        if let Some(feels) = current.apparent_temperature {
            info = info.push(text::caption(fl!(
                "feels-like",
                temp = model::temperature_precise(feels)
            )));
        }
        let mut row = widget::Row::new()
            .push(
                icon::from_name(model::icon(code, current.is_day != Some(0)))
                    .size(48)
                    .icon(),
            )
            .push(info.width(Length::Fill))
            .spacing(16)
            .align_y(Alignment::Center);
        if let Some(today) = model::days(&data.forecast, now).first() {
            let sun = |name: &'static str, time: Option<chrono::NaiveTime>| {
                widget::Row::new()
                    .push(icon::from_name(name).size(16).icon())
                    .push(text::body(time.map_or_else(String::new, model::clock)))
                    .spacing(8)
                    .align_y(Alignment::Center)
            };
            row = row.push(
                widget::Column::new()
                    .push(sun("daytime-sunrise-symbolic", today.sunrise))
                    .push(sun("daytime-sunset-symbolic", today.sunset))
                    .spacing(6),
            );
        }
        row.into()
    }

    /// Chart and table on one grid: both fill the same width, the chart is drawn at exactly
    /// that width, so each column sits under its point on the curve.
    fn view_timeline<'a>(&'a self, timeline: Timeline, units: Units) -> Element<'a, Message> {
        let columns = timeline.columns.clone();
        let chart = widget::container(responsive(move |size| {
            self.view_chart(&timeline, units, size.width)
        }))
        .height(Length::Fixed(chart::HEIGHT as f32))
        .width(Length::Fill);

        let table = columns.into_iter().fold(widget::Row::new(), |row, column| {
            let probability = column
                .probability
                .map_or_else(String::new, |p| format!("{p:.0}%"));
            row.push(
                widget::Column::new()
                    .push(text::body(
                        column
                            .temperature
                            .map_or_else(|| "–".to_owned(), model::temperature),
                    ))
                    .push(
                        icon::from_name(model::icon(column.code, column.is_day))
                            .size(16)
                            .icon(),
                    )
                    .push(
                        text::caption(probability)
                            .class(cosmic::theme::Text::Accent)
                            .height(Length::Fixed(16.0)),
                    )
                    .push(text::caption(
                        column
                            .hour
                            .map_or_else(|| fl!("now"), |h| format!("{h:02}")),
                    ))
                    .spacing(3)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
        });

        widget::Column::new()
            .push(chart)
            .push(table)
            .spacing(6)
            .into()
    }

    fn view_chart(&self, timeline: &Timeline, units: Units, width: f32) -> Element<'_, Message> {
        let key = ChartKey {
            width: width.max(1.0).floor() as u32,
            timeline: timeline.clone(),
            colors: chart::Colors::from_theme(&cosmic::theme::active()),
            units,
        };
        let mut cache = self.chart.borrow_mut();
        let handle = match &*cache {
            Some((cached, handle)) if *cached == key => handle.clone(),
            _ => {
                let mm = if units == Units::Imperial { 25.4 } else { 1.0 };
                let svg = chart::svg(timeline, &key.colors, mm, f64::from(key.width));
                let handle = widget::svg::Handle::from_memory(svg.into_bytes());
                *cache = Some((key.clone(), handle.clone()));
                handle
            }
        };
        widget::svg(handle)
            .width(Length::Fixed(key.width as f32))
            .height(Length::Fixed(chart::HEIGHT as f32))
            .into()
    }

    fn view_days(&self, now: chrono::NaiveDateTime, units: Units) -> Element<'_, Message> {
        let days = self.days();
        let range = model::range(&days);
        let mut col = widget::Column::new().push(padded_control(group_heading(fl!("next-days"))));
        for day in days {
            let open = self.expanded == Some(day.date);
            let name = if day.date == now.date() {
                fl!("today")
            } else {
                fl!("weekday", day = model::weekday(day.date))
            };
            let precipitation = day
                .precipitation
                .filter(|p| *p > 0.0)
                .map_or_else(String::new, |p| model::precipitation(p, units));
            let temp = |t: Option<f64>| t.map_or_else(|| "–".to_owned(), model::temperature);
            let bar = match (range, day.min, day.max) {
                (Some((lo, hi)), Some(min), Some(max)) => {
                    range_bar((min - lo) / (hi - lo), (max - lo) / (hi - lo))
                }
                _ => widget::Space::new().width(Length::Fill).into(),
            };
            let row = widget::Row::new()
                .push(
                    text::body(name)
                        .font(cosmic::font::semibold())
                        .width(Length::Fixed(48.0)),
                )
                .push(icon::from_name(model::icon(day.code, true)).size(16).icon())
                .push(
                    widget::container(text::caption(precipitation))
                        .align_right(Length::Fixed(56.0)),
                )
                .push(
                    widget::container(text::body(temp(day.min)).class(muted_text()))
                        .align_right(Length::Fixed(32.0)),
                )
                .push(bar)
                .push(widget::container(text::body(temp(day.max))).align_right(Length::Fixed(32.0)))
                .push(
                    icon::from_name(if open {
                        "pan-down-symbolic"
                    } else {
                        "go-next-symbolic"
                    })
                    .size(16)
                    .icon(),
                )
                .spacing(8)
                .align_y(Alignment::Center);
            col = col.push(menu_button(row).on_press(Message::ToggleDay(day.date)));
            if open {
                col = col.push(view_hours(&day, units));
            }
        }
        col.into()
    }
}

fn view_alert(alert: &Alert, units: Units) -> Element<'static, Message> {
    let what = fl!(
        "precip",
        kind = alert.kind.key(),
        intensity = alert.intensity.key()
    );
    let mut message = match alert.when {
        When::Starts(at) => fl!("alert-starts", what = what, time = model::clock(at.time())),
        When::Until(at) => fl!("alert-until", what = what, time = model::clock(at.time())),
        When::Ongoing => fl!("alert-ongoing", what = what),
    };
    if alert.snow > 0.0 {
        message = fl!(
            "alert-snow",
            alert = message,
            amount = model::snow(alert.snow, units)
        );
    }
    if alert.kind == model::Precip::FreezingRain {
        message = fl!("alert-ice", alert = message);
    }
    let class = if alert.kind.is_severe() {
        warning_text()
    } else {
        cosmic::theme::Text::Accent
    };
    widget::Row::new()
        .push(icon::from_name(alert.kind.icon()).size(16).icon())
        .push(
            text::body(message)
                .font(cosmic::font::semibold())
                .class(class),
        )
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// An opened day: 3-hour steps with temperature, rain and wind.
fn view_hours(day: &Day, units: Units) -> Element<'static, Message> {
    let rows = day.hours.iter().fold(widget::Column::new(), |col, hour| {
        col.push(
            widget::Row::new()
                .push(text::body(model::clock(hour.time)).width(Length::Fixed(48.0)))
                .push(
                    icon::from_name(model::icon(hour.code, hour.is_day))
                        .size(16)
                        .icon(),
                )
                .push(
                    text::body(
                        hour.temperature
                            .map_or_else(|| "–".to_owned(), model::temperature_precise),
                    )
                    .font(cosmic::font::semibold())
                    .width(Length::Fill),
                )
                .push(text::caption(if hour.precipitation > 0.0 {
                    model::precipitation(hour.precipitation, units)
                } else {
                    String::new()
                }))
                .push(
                    widget::container(text::caption(
                        hour.wind
                            .map_or_else(String::new, |w| model::wind(w, units)),
                    ))
                    .align_right(Length::Fixed(64.0)),
                )
                .spacing(8)
                .align_y(Alignment::Center)
                .height(Length::Fixed(28.0)),
        )
    });
    widget::container(rows)
        .padding([4, 24])
        .width(Length::Fill)
        .class(cosmic::theme::Container::custom(|theme| {
            iced_container::Style {
                background: Some(Background::Color(
                    theme
                        .cosmic()
                        .background(theme.transparent)
                        .component
                        .base
                        .into(),
                )),
                ..Default::default()
            }
        }))
        .into()
}

fn view_air(air: &AirCurrent, uv: Option<f64>) -> Element<'static, Message> {
    let mut col = widget::Column::new()
        .push(group_heading(fl!("air")))
        .spacing(8);
    if let Some(aqi) = air.european_aqi {
        let band = model::aqi_band(aqi);
        col = col.push(meter(
            fl!("aqi"),
            format!("{aqi:.0} · {}", fl!("aqi-band", band = band)),
            (aqi / 100.0).min(1.0),
            &AQI_COLORS,
        ));
    }
    let value = |v: Option<f64>| v.map_or_else(|| "–".to_owned(), |v| format!("{v:.0} µg/m³"));
    col = col.push(
        widget::Row::new()
            .push(tile(fl!("pm25"), value(air.pm2_5)))
            .push(tile(fl!("no2"), value(air.nitrogen_dioxide)))
            .spacing(8),
    );
    if let Some(uv) = uv {
        col = col.push(meter(
            fl!("uv-max"),
            format!("{uv:.1} · {}", fl!("uv-band", band = model::uv_band(uv))),
            (uv / 11.0).min(1.0),
            &UV_COLORS,
        ));
    }
    col.into()
}

fn view_pollen(air: &AirCurrent) -> Element<'static, Message> {
    let mut col = widget::Column::new()
        .push(group_heading(fl!("pollen")))
        .spacing(4);
    if Pollen::ALL.iter().all(|p| p.value(air).is_none()) {
        return col.push(text::caption(fl!("pollen-unavailable"))).into();
    }
    for pollen in Pollen::ALL {
        let Some(grains) = pollen.value(air) else {
            continue;
        };
        let level = pollen.level(grains);
        let pips = (1..=3).fold(widget::Row::new().spacing(3), |row, n| {
            row.push(swatch(
                if n <= level {
                    warning_color()
                } else {
                    track_color()
                },
                14.0,
                6.0,
            ))
        });
        col = col.push(
            widget::Row::new()
                .push(text::body(fl!("pollen-name", kind = pollen.key())).width(Length::Fill))
                .push(pips)
                .push(
                    widget::container(text::caption(fl!("pollen-level", level = level)))
                        .align_right(Length::Fixed(72.0)),
                )
                .align_y(Alignment::Center),
        );
    }
    col.into()
}

// ---------------------------------------------------------------- popup: settings

impl AppModel {
    fn view_settings(&self) -> Element<'_, Message> {
        let mut locations = widget::Column::new();
        for (index, location) in self.config.locations.iter().enumerate() {
            let active = self.config.location() == Some(location);
            let mut detail = location.detail.clone();
            if let Some(elevation) = location.elevation {
                if !detail.is_empty() {
                    detail.push_str(" · ");
                }
                detail.push_str(&fl!("elevation", metres = format!("{elevation:.0}")));
            }
            let row = widget::Row::new()
                .push(
                    widget::container(if active {
                        Element::from(icon::from_name("object-select-symbolic").size(16).icon())
                    } else {
                        widget::Space::new().width(16).into()
                    })
                    .width(Length::Fixed(16.0)),
                )
                .push(
                    widget::Column::new()
                        .push(text::body(location.name.clone()))
                        .push(text::caption(detail))
                        .width(Length::Fill),
                )
                .push(icon_button("edit-delete-symbolic").on_press(Message::DeleteLocation(index)))
                .spacing(12)
                .align_y(Alignment::Center);
            locations = locations.push(menu_button(row).on_press(Message::SelectLocation(index)));
        }
        locations = locations.push(
            menu_button(
                widget::Row::new()
                    .push(text::body(fl!("add-location")).width(Length::Fill))
                    .push(icon::from_name("list-add-symbolic").size(16).icon())
                    .align_y(Alignment::Center),
            )
            .on_press(Message::OpenPage(Page::Search)),
        );

        let units = |label: String, units: Units| {
            let msg = Message::SetUnits(units);
            if self.config.units == units {
                button::suggested(label).on_press(msg)
            } else {
                button::standard(label).on_press(msg)
            }
        };
        let refresh = REFRESH_CHOICES
            .iter()
            .position(|m| *m == self.config.refresh_mins());

        let options = widget::Column::new()
            .push(group_heading(fl!("units")))
            .push(
                widget::Row::new()
                    .push(units(fl!("units-metric"), Units::Metric))
                    .push(units(fl!("units-imperial"), Units::Imperial))
                    .spacing(8),
            )
            .push(group_heading(fl!("panel")))
            .push(toggle_row(
                fl!("panel-temperature"),
                None,
                self.config.panel_temperature,
                Message::SetPanelTemperature,
            ))
            .push(toggle_row(
                fl!("panel-precipitation"),
                None,
                self.config.panel_precipitation,
                Message::SetPanelPrecipitation,
            ))
            .push(toggle_row(
                fl!("panel-high-low"),
                None,
                self.config.panel_high_low,
                Message::SetPanelHighLow,
            ))
            .push(group_heading(fl!("popup")))
            .push(toggle_row(
                fl!("show-air"),
                Some(fl!("show-air-hint")),
                self.config.show_air,
                Message::SetShowAir,
            ))
            .push(toggle_row(
                fl!("show-pollen"),
                Some(fl!("show-pollen-hint")),
                self.config.show_pollen,
                Message::SetShowPollen,
            ))
            .push(
                widget::Row::new()
                    .push(text::body(fl!("refresh-every")).width(Length::Fill))
                    .push(widget::dropdown(
                        self.refresh_labels.as_slice(),
                        refresh,
                        Message::SetRefresh,
                    ))
                    .align_y(Alignment::Center),
            )
            .spacing(10);

        let body = widget::Column::new()
            .push(padded_control(group_heading(fl!("locations"))))
            .push(locations)
            .push(section_divider())
            .push(padded_control(options));

        widget::Column::new()
            .padding([8, 0])
            .push(padded_control(page_header(fl!("settings"), Page::Weather)))
            .push(section_divider())
            .push(
                widget::container(widget::scrollable(body).height(Length::Shrink))
                    .max_height(POPUP_MAX_HEIGHT),
            )
            .into()
    }

    fn view_search(&self) -> Element<'_, Message> {
        let search = &self.search;
        let back = if self.config.locations.is_empty() {
            Page::Weather
        } else {
            Page::Settings
        };
        let input = widget::Row::new()
            .push(
                widget::text_input(fl!("search-placeholder"), search.query.clone())
                    .on_input(Message::SearchQuery)
                    .on_submit(|_| Message::Search)
                    .width(Length::Fill),
            )
            .push(button::standard(fl!("search")).on_press_maybe(
                (!search.searching && !search.query.trim().is_empty()).then_some(Message::Search),
            ))
            .spacing(8)
            .align_y(Alignment::Center);

        let mut results = widget::Column::new();
        match &search.results {
            _ if search.searching => {
                results = results.push(padded_control(text::caption(fl!("searching"))));
            }
            Some(Ok(places)) if places.is_empty() => {
                results = results.push(padded_control(text::caption(fl!(
                    "no-results",
                    query = search.searched.clone()
                ))));
            }
            Some(Ok(places)) => {
                for place in places {
                    let mut detail = place.detail.clone();
                    if let Some(elevation) = place.elevation {
                        if !detail.is_empty() {
                            detail.push_str(" · ");
                        }
                        detail.push_str(&fl!("elevation", metres = format!("{elevation:.0}")));
                    }
                    results = results.push(
                        menu_button(
                            widget::Row::new()
                                .push(
                                    widget::Column::new()
                                        .push(text::body(place.name.clone()))
                                        .push(text::caption(detail))
                                        .width(Length::Fill),
                                )
                                .push(icon::from_name("list-add-symbolic").size(16).icon())
                                .align_y(Alignment::Center),
                        )
                        .on_press(Message::AddLocation(place.clone())),
                    );
                }
            }
            Some(Err(err)) => {
                results = results.push(padded_control(error_text(fl!(
                    "search-error",
                    error = describe(err)
                ))));
            }
            None => {}
        }

        let coordinates = widget::Column::new()
            .push(text::caption(fl!("or-coordinates")))
            .push(
                widget::Row::new()
                    .push(
                        widget::text_input(fl!("latitude"), search.latitude.clone())
                            .on_input(Message::Latitude)
                            .width(Length::Fill),
                    )
                    .push(
                        widget::text_input(fl!("longitude"), search.longitude.clone())
                            .on_input(Message::Longitude)
                            .on_submit(|_| Message::AddCoordinates)
                            .width(Length::Fill),
                    )
                    .push(
                        button::suggested(fl!("add"))
                            .on_press_maybe(search.coordinates().map(|_| Message::AddCoordinates)),
                    )
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .spacing(6);

        widget::Column::new()
            .padding([8, 0])
            .push(padded_control(page_header(fl!("add-location"), back)))
            .push(section_divider())
            .push(padded_control(input))
            .push(
                widget::container(widget::scrollable(results).height(Length::Shrink))
                    .max_height(360.0),
            )
            .push(padded_control(text::caption(fl!("search-hint"))))
            .push(section_divider())
            .push(padded_control(coordinates))
            .into()
    }
}

// ---------------------------------------------------------------- small widgets

fn page_header(title: String, back: Page) -> Element<'static, Message> {
    widget::Row::new()
        .push(icon_button("go-previous-symbolic").on_press(Message::OpenPage(back)))
        .push(text::heading(title))
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn group_heading(label: String) -> Element<'static, Message> {
    text::caption_heading(label).class(muted_text()).into()
}

fn toggle_row(
    label: String,
    hint: Option<String>,
    on: bool,
    msg: fn(bool) -> Message,
) -> Element<'static, Message> {
    let mut info = widget::Column::new().push(text::body(label));
    if let Some(hint) = hint {
        info = info.push(text::caption(hint));
    }
    widget::Row::new()
        .push(info.width(Length::Fill))
        .push(widget::toggler(on).on_toggle(msg))
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// Label and value on a rounded component background.
fn tile(label: String, value: String) -> Element<'static, Message> {
    widget::container(
        widget::Column::new()
            .push(text::caption(label))
            .push(text::body(value).font(cosmic::font::semibold())),
    )
    .padding([6, 10])
    .width(Length::Fill)
    .class(cosmic::theme::Container::custom(|theme| {
        iced_container::Style {
            background: Some(Background::Color(
                theme
                    .cosmic()
                    .background(theme.transparent)
                    .component
                    .base
                    .into(),
            )),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }))
    .into()
}

/// European AQI bands, good to very poor (beyond 100 the marker sits at the end).
const AQI_COLORS: [(u8, u8, u8); 5] = [
    (0x50, 0xc8, 0x78),
    (0xc8, 0xd6, 0x4a),
    (0xf2, 0xc9, 0x4c),
    (0xf2, 0x99, 0x4a),
    (0xeb, 0x57, 0x57),
];
/// UV bands on a 0–11 scale: low (<3), moderate, high, very high, extreme (11+).
const UV_COLORS: [(u8, u8, u8); 5] = [
    (0x50, 0xc8, 0x78),
    (0xf2, 0xc9, 0x4c),
    (0xf2, 0x99, 0x4a),
    (0xeb, 0x57, 0x57),
    (0x9b, 0x51, 0xe0),
];

/// A label with its value, over a banded colour bar with a marker at `position` (0–1).
fn meter(
    label: String,
    value: String,
    position: f64,
    colors: &[(u8, u8, u8); 5],
) -> Element<'static, Message> {
    let bands = colors
        .iter()
        .fold(widget::Row::new().spacing(2), |row, (r, g, b)| {
            row.push(swatch(Color::from_rgb8(*r, *g, *b), 0.0, 6.0))
        });
    let at = (position.clamp(0.0, 1.0) * 1000.0).round() as u16;
    let marker = widget::Row::new()
        .push(widget::Space::new().width(Length::FillPortion(at.max(1))))
        .push(swatch(on_background(), 4.0, 14.0))
        .push(widget::Space::new().width(Length::FillPortion((1000 - at).max(1))));
    widget::Column::new()
        .push(
            widget::Row::new()
                .push(text::body(label).width(Length::Fill))
                .push(text::body(value).font(cosmic::font::semibold())),
        )
        .push(stack![
            widget::container(bands).padding([4, 0]).width(Length::Fill),
            marker
        ])
        .spacing(4)
        .into()
}

/// The day's low-to-high span on the shared scale (both 0–1).
fn range_bar(low: f64, high: f64) -> Element<'static, Message> {
    let portion = |v: f64| (v.clamp(0.0, 1.0) * 1000.0).round() as u16;
    let (a, b) = (portion(low), portion(high).max(portion(low) + 20).min(1000));
    let mut row = widget::Row::new();
    if a > 0 {
        row = row.push(widget::Space::new().width(Length::FillPortion(a)));
    }
    row = row.push(swatch_fill(accent_color(), Length::FillPortion(b - a), 5.0));
    if b < 1000 {
        row = row.push(widget::Space::new().width(Length::FillPortion(1000 - b)));
    }
    widget::container(row)
        .width(Length::Fill)
        .class(rounded(track_color()))
        .into()
}

/// A rounded block of colour; width 0 means fill.
fn swatch(color: Color, width: f32, height: f32) -> Element<'static, Message> {
    let width = if width > 0.0 {
        Length::Fixed(width)
    } else {
        Length::Fill
    };
    swatch_fill(color, width, height)
}

fn swatch_fill(color: Color, width: Length, height: f32) -> Element<'static, Message> {
    widget::container(
        widget::Space::new()
            .width(width)
            .height(Length::Fixed(height)),
    )
    .width(width)
    .class(rounded(color))
    .into()
}

fn rounded(color: Color) -> cosmic::theme::Container<'static> {
    cosmic::theme::Container::custom(move |_| iced_container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: 3.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn accent_color() -> Color {
    cosmic::theme::active().cosmic().accent_color().into()
}

fn warning_color() -> Color {
    cosmic::theme::active().cosmic().warning_color().into()
}

fn on_background() -> Color {
    cosmic::theme::active().cosmic().on_bg_color().into()
}

fn track_color() -> Color {
    let mut color = on_background();
    color.a = 0.2;
    color
}

fn muted_text() -> cosmic::theme::Text {
    let mut color = on_background();
    color.a = 0.65;
    cosmic::theme::Text::Color(color)
}

fn warning_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(warning_color())
}

fn describe(err: &ApiError) -> String {
    match err {
        ApiError::Unreachable(e) => fl!("error-unreachable", error = e.clone()),
        ApiError::RateLimited => fl!("error-rate-limited"),
        ApiError::Rejected(e) => fl!("error-rejected", error = e.clone()),
        ApiError::Http(code) => fl!("error-http", code = code),
        ApiError::Protocol(e) => fl!("error-protocol", error = e.clone()),
    }
}
