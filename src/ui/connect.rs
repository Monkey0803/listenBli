//! 接入来源 — declaring a new source.
//!
//! The design's idea, and the reason this is a form rather than a message: a
//! source *is* its capability declaration, and the interface grows whatever that
//! declaration asks for. What this version cannot do is fetch from one — there is
//! no implementation behind a declaration — so the sheet says that on the way in
//! and 来源与同步 says it again on every card.

use egui::{Align, Align2, Color32, Id, Layout, Margin, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::api::models::{Source, CAPABILITIES, CAPABILITY_GROUPS};
use crate::app::{App, ConnectDraft};
use crate::config::{Declaration, Session};

use super::icons;
use super::theme::{self as t};
use super::widgets::{self, Emphasis};

const SHEET_W: f32 = 700.0;
const PAD: f32 = 20.0;
/// Tall enough that the three steps are usually all visible; the scroll area is
/// for the cases where they are not.
const BODY_MAX_H: f32 = 620.0;

/// What the sheet's buttons asked for, applied once the modal has closed.
enum Ask {
    Close,
    Create,
}

impl App {
    pub(crate) fn ui_connect_sheet(&mut self, ctx: &egui::Context) {
        let accent = self.theme.accent;
        let taken = self.taken_channels();
        let mut ask: Option<Ask> = None;

        let response = egui::Modal::new(Id::new("connect-sheet"))
            .frame(
                egui::Frame::NONE
                    .fill(t::INK_2)
                    .corner_radius(t::R_MD)
                    .inner_margin(Margin::ZERO)
                    .stroke(Stroke::new(1.0, t::LINE_2)),
            )
            .backdrop_color(Color32::from_black_alpha(160))
            .show(ctx, |ui| {
                ui.set_width(SHEET_W);
                self.connect_head(ui, accent, &mut ask);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(BODY_MAX_H)
                    .show(ui, |ui| {
                        ui.add_space(16.0);
                        self.connect_basics(ui, accent, &taken);
                        ui.add_space(18.0);
                        self.connect_capabilities(ui, accent);
                        ui.add_space(18.0);
                        self.connect_session(ui, accent);
                        ui.add_space(18.0);
                    });

                self.connect_foot(ui, accent, &taken, &mut ask);
            });

        if response.should_close() {
            ask = Some(Ask::Close);
        }
        match ask {
            Some(Ask::Close) => self.connect.open = false,
            Some(Ask::Create) => self.create_declaration(),
            None => {}
        }
    }

    /// The letters already in use: the built-in sources' and every declaration's.
    fn taken_channels(&self) -> Vec<String> {
        let mut taken: Vec<String> = Source::ALL
            .iter()
            .map(|source| source.channel().to_owned())
            .collect();
        for declaration in crate::config::with(&self.config, |config| config.declared.clone()) {
            taken.push(declaration.channel);
        }
        taken
    }

    fn connect_head(&self, ui: &mut Ui, accent: t::Accent, ask: &mut Option<Ask>) {
        let (head, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, 64.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.hline(
            head.x_range(),
            head.bottom() - 0.5,
            Stroke::new(1.0, t::LINE),
        );
        let badge = Rect::from_min_size(
            Pos2::new(head.left() + PAD, head.top() + 17.0),
            Vec2::splat(28.0),
        );
        painter.rect_filled(badge, t::R_SM, t::INK_3);
        painter.rect_stroke(
            badge,
            t::R_SM,
            Stroke::new(1.0, t::LINE_2),
            egui::StrokeKind::Inside,
        );
        icons::plug(
            &painter,
            Rect::from_center_size(badge.center(), Vec2::splat(15.0)),
            t::FG_2,
        );
        widgets::label_at_bold(
            ui,
            Pos2::new(badge.right() + 12.0, badge.center().y - 7.0),
            Align2::LEFT_CENTER,
            "接入新来源",
            t::ui_font(15.0),
            t::FG,
        );
        widgets::label_at(
            ui,
            Pos2::new(badge.right() + 12.0, badge.center().y + 11.0),
            Align2::LEFT_CENTER,
            "写入一份能力声明，界面据此长出对应品目",
            t::mono_font(10.0),
            t::FG_4,
        );
        let close = Rect::from_min_size(
            Pos2::new(head.right() - PAD - 26.0, head.top() + 19.0),
            Vec2::splat(26.0),
        );
        if widgets::icon_button_at(
            ui,
            Id::new("connect-close"),
            close,
            icons::close,
            false,
            accent,
            "关闭",
        )
        .clicked()
        {
            *ask = Some(Ask::Close);
        }
    }

    /// 01 基本信息: what it is called, and the letter lists will badge it with.
    fn connect_basics(&mut self, ui: &mut Ui, accent: t::Accent, taken: &[String]) {
        step_label(ui, "01", "基本信息");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(PAD);
            field_label(ui, "名称");
            let mut typed = self.connect.name.clone();
            let response = widgets::text_field(ui, &mut typed, "例如：某个音乐平台", 250.0, accent);
            if response.changed() {
                self.connect.name = typed;
            }
            ui.add_space(16.0);
            field_label(ui, "通道字母");
            let mut letter = self.connect.channel.clone();
            let response = widgets::text_field(ui, &mut letter, "C", 44.0, accent);
            if response.changed() {
                // One letter, uppercased: it is a badge, not a name.
                let mut chars = letter.chars().filter(|c| !c.is_whitespace());
                self.connect.channel = chars
                    .next()
                    .map(|c| c.to_ascii_uppercase().to_string())
                    .unwrap_or_default();
            }

            // The availability line is part of this row, so it is laid out after
            // the fields rather than placed at a guessed x.
            ui.add_space(14.0);
            let channel = self.connect.channel.trim().to_owned();
            let occupied = !channel.is_empty() && taken.iter().any(|used| used == &channel);
            let hint = match channel.as_str() {
                "" => "列表里会显示为 字母·条目".to_owned(),
                letter if occupied => format!("通道 {letter} 已被占用"),
                letter => format!("通道 {letter} 可用 · 列表里会显示为 {letter}·条目"),
            };
            let font = t::mono_font(10.0);
            let width = widgets::measure(ui, &hint, &font).x + 4.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
            widgets::paint_label(
                ui.painter(),
                rect.left_center(),
                Align2::LEFT_CENTER,
                &hint,
                font,
                if occupied { t::RED } else { t::FG_4 },
            );
        });
    }

    /// 02 能力声明: the four groups, each item showing what it would grow.
    fn connect_capabilities(&mut self, ui: &mut Ui, accent: t::Accent) {
        step_label(ui, "02", "能力声明");
        ui.add_space(8.0);
        for group in CAPABILITY_GROUPS {
            ui.horizontal(|ui| {
                ui.add_space(PAD);
                widgets::engraved_at(
                    ui,
                    Pos2::new(ui.cursor().min.x, ui.cursor().min.y + 8.0),
                    Align2::LEFT_CENTER,
                    group,
                    9.0,
                    t::FG_4,
                );
                ui.add_space(widgets::engraved_measure(ui, group, 9.0).x);
            });
            ui.add_space(4.0);
            for capability in CAPABILITIES.iter().filter(|cap| cap.group == group) {
                let on = self.connect.declares(capability.key);
                if capability_row(ui, capability.label, capability.surface, on, accent) {
                    self.connect.toggle(capability.key);
                }
            }
            ui.add_space(10.0);
        }
    }

    /// 03 会话方式.
    fn connect_session(&mut self, ui: &mut Ui, accent: t::Accent) {
        step_label(ui, "03", "会话方式");
        ui.add_space(8.0);
        for session in [Session::Anonymous, Session::Account] {
            if option_row(
                ui,
                session.label(),
                session.detail(),
                self.connect.session == session,
                accent,
            ) {
                self.connect.session = session;
            }
        }
    }

    fn connect_foot(
        &self,
        ui: &mut Ui,
        accent: t::Accent,
        taken: &[String],
        ask: &mut Option<Ask>,
    ) {
        let (foot, _) = ui.allocate_exact_size(Vec2::new(SHEET_W, 54.0), Sense::hover());
        let painter = ui.painter().clone();
        painter.hline(foot.x_range(), foot.top() + 0.5, Stroke::new(1.0, t::LINE));

        let chosen = self.connect.capabilities.len();
        let missing = CAPABILITIES.len().saturating_sub(chosen);
        let problem = self.connect.problem(taken);
        widgets::label_at(
            ui,
            Pos2::new(foot.left() + PAD, foot.center().y),
            Align2::LEFT_CENTER,
            &problem
                .clone()
                .unwrap_or_else(|| format!("{chosen} 项能力 · 缺 {missing} 项")),
            t::mono_font(10.0),
            if problem.is_some() { t::RED } else { t::FG_4 },
        );

        let actions = Rect::from_min_max(
            Pos2::new(foot.right() - PAD - 240.0, foot.center().y - 13.0),
            Pos2::new(foot.right() - PAD, foot.center().y + 13.0),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(actions)
                .layout(Layout::right_to_left(Align::Center)),
        );
        if widgets::console_button(&mut child, "取消", None, Emphasis::Ghost, accent).clicked() {
            *ask = Some(Ask::Close);
        }
        child.add_space(8.0);
        // Drawn quiet while something is missing, so the button says "not yet"
        // rather than looking ready and doing nothing.
        let create = widgets::console_button(
            &mut child,
            "创建声明",
            Some(icons::check),
            if problem.is_none() {
                Emphasis::Primary
            } else {
                Emphasis::Ghost
            },
            accent,
        );
        if create.clicked() && problem.is_none() {
            *ask = Some(Ask::Create);
        }
    }

    /// Writes the declaration and says exactly what it does and does not do.
    fn create_declaration(&mut self) {
        let name = self.connect.name.trim().to_owned();
        let channel = self.connect.channel.trim().to_owned();
        let capabilities = self.connect.capabilities.clone();
        let session = self.connect.session;
        let existing: Vec<String> =
            crate::config::with(&self.config, |config| config.declared.clone())
                .into_iter()
                .map(|declaration| declaration.id)
                .collect();
        let id = Declaration::id_for(&name, &existing);
        self.declare(Declaration {
            id,
            name: name.clone(),
            channel: channel.clone(),
            capabilities,
            session,
        });
        self.connect.open = false;
        self.set_status(
            format!(
                "已写入声明：{name}（通道 {channel}）· 本版本还没有它的实现，所以它不会提供任何内容"
            ),
            false,
        );
    }
}

impl ConnectDraft {
    /// Whether a capability is on in the draft.
    pub(crate) fn declares(&self, key: &str) -> bool {
        self.capabilities.iter().any(|declared| declared == key)
    }

    /// Adds or removes a capability.
    pub(crate) fn toggle(&mut self, key: &str) {
        match self
            .capabilities
            .iter()
            .position(|declared| declared == key)
        {
            Some(index) => {
                self.capabilities.remove(index);
            }
            None => self.capabilities.push(key.to_owned()),
        }
    }

    /// What is missing before this can be created, if anything.
    pub(crate) fn problem(&self, taken: &[String]) -> Option<String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Some("需要一个名称".to_owned());
        }
        let channel = self.channel.trim();
        if channel.is_empty() {
            return Some("需要一个通道字母".to_owned());
        }
        if taken.iter().any(|used| used == channel) {
            return Some(format!("通道 {channel} 已被占用"));
        }
        if self.capabilities.is_empty() {
            return Some("至少声明一项能力".to_owned());
        }
        None
    }
}

/// The `NN 标签` line that opens a step.
fn step_label(ui: &mut Ui, number: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        widgets::label_at(
            ui,
            Pos2::new(ui.cursor().min.x, ui.cursor().min.y + 8.0),
            Align2::LEFT_CENTER,
            number,
            t::mono_font(11.0),
            t::FG_4,
        );
        ui.add_space(22.0);
        widgets::engraved_at(
            ui,
            Pos2::new(ui.cursor().min.x, ui.cursor().min.y + 8.0),
            Align2::LEFT_CENTER,
            label,
            9.5,
            t::FG_3,
        );
        ui.add_space(widgets::engraved_measure(ui, label, 9.5).x);
    });
}

/// A small label above a field.
fn field_label(ui: &mut Ui, label: &str) {
    widgets::engraved_at(
        ui,
        Pos2::new(ui.cursor().min.x, ui.cursor().min.y + 16.0),
        Align2::LEFT_CENTER,
        label,
        9.0,
        t::FG_4,
    );
    ui.add_space(widgets::engraved_measure(ui, label, 9.0).x + 8.0);
}

/// One capability, with what turning it on would grow.
fn capability_row(ui: &mut Ui, label: &str, surface: &str, on: bool, accent: t::Accent) -> bool {
    let (row, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width() - PAD * 2.0, 30.0),
        Sense::click(),
    );
    let rect = row.translate(Vec2::new(PAD, 0.0));
    let painter = ui.painter().clone();
    if response.hovered() {
        painter.rect_filled(rect, t::R_SM, t::INK_3);
    }

    // A square box rather than a tick: the design's `mx__box`, which reads as a
    // checklist and not as a switch.
    let box_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 4.0, rect.center().y - 7.0),
        Vec2::splat(14.0),
    );
    if on {
        painter.rect_filled(box_rect, t::R_SM, t::fade(accent.accent, 0.22));
    }
    painter.rect_stroke(
        box_rect,
        t::R_SM,
        Stroke::new(
            1.0,
            if on {
                t::fade(accent.accent, 0.6)
            } else {
                t::LINE_2
            },
        ),
        egui::StrokeKind::Inside,
    );
    if on {
        icons::check(
            &painter,
            Rect::from_center_size(box_rect.center(), Vec2::splat(11.0)),
            accent.accent,
        );
    }

    widgets::label_at(
        ui,
        Pos2::new(box_rect.right() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.0),
        if on { t::FG } else { t::FG_3 },
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.left() + 190.0, rect.center().y),
        Align2::LEFT_CENTER,
        surface,
        t::mono_font(9.5),
        t::FG_4,
    );
    widgets::label_at(
        ui,
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        if on { "会启用" } else { "不启用" },
        t::mono_font(9.5),
        if on { accent.accent } else { t::FG_4 },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// One session choice.
fn option_row(ui: &mut Ui, label: &str, detail: &str, on: bool, accent: t::Accent) -> bool {
    let (row, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width() - PAD * 2.0, 44.0),
        Sense::click(),
    );
    let rect = row.translate(Vec2::new(PAD, 0.0));
    let painter = ui.painter().clone();
    painter.rect_filled(
        rect,
        t::R_SM,
        if on {
            t::fade(accent.accent, 0.08)
        } else if response.hovered() {
            t::INK_3
        } else {
            Color32::TRANSPARENT
        },
    );
    painter.rect_stroke(
        rect,
        t::R_SM,
        Stroke::new(
            1.0,
            if on {
                t::fade(accent.accent, 0.4)
            } else {
                t::LINE
            },
        ),
        egui::StrokeKind::Inside,
    );
    let dot = Pos2::new(rect.left() + 14.0, rect.center().y);
    painter.circle_stroke(
        dot,
        6.0,
        Stroke::new(1.0, if on { accent.accent } else { t::LINE_3 }),
    );
    if on {
        painter.circle_filled(dot, 3.0, accent.accent);
    }
    widgets::label_at(
        ui,
        Pos2::new(dot.x + 16.0, rect.top() + 16.0),
        Align2::LEFT_CENTER,
        label,
        t::ui_font(12.0),
        if on { t::FG } else { t::FG_2 },
    );
    widgets::label_at(
        ui,
        Pos2::new(dot.x + 16.0, rect.top() + 33.0),
        Align2::LEFT_CENTER,
        detail,
        t::mono_font(9.5),
        t::FG_4,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use egui::{Pos2, RawInput, Rect, Vec2};

    fn raw() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
            ..Default::default()
        }
    }

    fn texts(output: &egui::FullOutput) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for shape in &output.shapes {
            walk(&shape.shape, &mut out);
        }
        out
    }

    fn an_app() -> (App, egui::Context) {
        crate::platform::serialise_home_tests();
        std::env::set_var("HOME", "/tmp/listenbli-connect-tests");
        let _ = std::fs::create_dir_all("/tmp/listenbli-connect-tests");
        let mut app = App::new_for_tests(Config::default());
        // These tests share one settings file, and one of them writes a
        // declaration to it, so each starts from none.
        app.update_config(|config| config.declared.clear());
        app.open_connect_sheet();

        let ctx = egui::Context::default();
        crate::ui::theme::install_fonts(&ctx, None);
        crate::ui::theme::install_style(&ctx, &app.theme);
        (app, ctx)
    }

    /// A modal lays out over more than one pass, so read it once it is not empty.
    fn sheet(app: &mut App, ctx: &egui::Context) -> Vec<String> {
        let mut painted = Vec::new();
        for _ in 0..4 {
            let mut output = ctx.run_ui(raw(), |ui| app.ui_connect_sheet(ui.ctx()));
            output.textures_delta.clear();
            painted = texts(&output);
            if !painted.is_empty() {
                break;
            }
        }
        painted
    }

    #[test]
    fn the_form_asks_for_the_three_things_a_declaration_needs() {
        let (mut app, ctx) = an_app();
        let painted = sheet(&mut app, &ctx);
        for wanted in [
            "接入新来源",
            "01",
            "基本信息",
            "02",
            "能力声明",
            "03",
            "会话方式",
        ] {
            assert!(
                painted.iter().any(|text| text == wanted),
                "{wanted} should be on the form: {painted:?}"
            );
        }
        // A capability shows what turning it on would grow, and the two states are
        // spelled out rather than implied by a tick.
        assert!(
            painted.iter().any(|text| text == "关键词搜索"),
            "capabilities are listed: {painted:?}"
        );
        assert!(
            painted.iter().any(|text| text == "搜索框与结果列表"),
            "with the part of the interface each one grows: {painted:?}"
        );
        assert!(painted.iter().any(|text| text == "会启用"));
        assert!(painted.iter().any(|text| text == "不启用"));
        assert!(
            painted.iter().any(|text| text == "匿名")
                && painted.iter().any(|text| text == "账号登录"),
            "and both ways of signing in: {painted:?}"
        );
    }

    /// The design's own example: 通道 B 已被占用. Ours are B and Y, so the second
    /// built-in source is the interesting one.
    #[test]
    fn a_channel_letter_already_in_use_is_refused() {
        let (mut app, ctx) = an_app();
        app.connect.channel = "Y".to_owned();
        let painted = sheet(&mut app, &ctx);
        assert!(
            painted.iter().any(|text| text == "通道 Y 已被占用"),
            "a taken letter says so: {painted:?}"
        );
        assert!(
            app.connect.problem(&app.taken_channels()).is_some(),
            "and cannot be created with"
        );

        app.connect.name = "Internet Archive".to_owned();
        app.connect.channel = "C".to_owned();
        let painted = sheet(&mut app, &ctx);
        assert!(
            painted.iter().any(|text| text.contains("通道 C 可用")),
            "a free one says what it will be used for: {painted:?}"
        );
        assert!(app.connect.problem(&app.taken_channels()).is_none());
    }

    #[test]
    fn an_unfinished_form_says_what_is_missing() {
        let (mut app, ctx) = an_app();
        // Fresh: the design's default set is on, but nothing has been named.
        let painted = sheet(&mut app, &ctx);
        assert!(
            painted.iter().any(|text| text == "需要一个名称"),
            "the form says what it is waiting for: {painted:?}"
        );

        app.connect.name = "Internet Archive".to_owned();
        assert_eq!(
            app.connect.problem(&app.taken_channels()),
            Some("需要一个通道字母".to_owned())
        );
        app.connect.channel = "I".to_owned();
        app.connect.capabilities.clear();
        assert_eq!(
            app.connect.problem(&app.taken_channels()),
            Some("至少声明一项能力".to_owned()),
            "a source that declares nothing would grow nothing"
        );
    }

    #[test]
    fn creating_writes_a_declaration_that_survives_a_restart() {
        let (mut app, _ctx) = an_app();
        app.connect.name = "Internet Archive".to_owned();
        app.connect.channel = "i".to_owned();
        app.connect.session = Session::Account;
        app.connect.capabilities = vec!["search".to_owned(), "stream".to_owned()];
        app.create_declaration();

        let written = crate::config::with(&app.config, |config| config.declared.clone());
        assert_eq!(written.len(), 1, "one declaration");
        let declaration = &written[0];
        assert_eq!(declaration.name, "Internet Archive");
        assert_eq!(
            declaration.id, "internet-archive",
            "the id comes from the name"
        );
        assert_eq!(declaration.channel, "i", "the letter is kept as typed");
        assert_eq!(declaration.declared_count(), 2);
        assert!(
            !declaration.declares("lossless"),
            "and only what was chosen"
        );
        assert_eq!(declaration.session, Session::Account);
        assert!(!app.connect.open, "the form closes behind it");

        // It is a setting, so it is in the file and not only in memory.
        let reloaded = Config::load();
        assert!(
            reloaded
                .declared
                .iter()
                .any(|declared| declared.id == "internet-archive"),
            "a declaration survives a restart: {reloaded:?}"
        );
    }

    /// The honest half: the app says out loud what a declaration cannot do.
    #[test]
    fn creating_says_the_source_is_not_implemented() {
        let (mut app, _ctx) = an_app();
        app.connect.name = "Internet Archive".to_owned();
        app.connect.channel = "I".to_owned();
        app.create_declaration();

        let status = app.status.as_ref().expect("creating reports what it did");
        assert!(
            status.text.contains("还没有它的实现"),
            "the status says the declaration has no implementation: {}",
            status.text
        );
        assert!(!status.is_error, "it is a note, not a failure");
    }

    #[test]
    fn a_capability_can_be_turned_on_and_off_again() {
        let draft = ConnectDraft::default();
        assert!(draft.declares("search"), "the design's starting set");
        assert!(!draft.declares("lossless"));

        let mut draft = draft;
        draft.toggle("lossless");
        assert!(draft.declares("lossless"));
        draft.toggle("lossless");
        assert!(!draft.declares("lossless"), "and off again");
        draft.toggle("search");
        assert!(!draft.declares("search"));
    }
}
