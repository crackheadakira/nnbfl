use std::collections::{HashMap, HashSet};

use nnbfl::bflan::{
    anim_info::{AnimInfo, AnimInfoType, AnimType, PaneAnimInfo},
    curves::Curve,
};

use crate::{
    anim_edit::{self, Animatable},
    anim_state::{AnimInstance, AnimPlayer, eval_curve},
    ui::DrawUi,
};

pub const TIMELINE_RULER_HEIGHT: f32 = 22.0;
pub const TIMELINE_MARKER_RADIUS: f32 = 4.0;

const LABEL_COL_WIDTH: f32 = 210.0;
const INSPECTOR_WIDTH: f32 = 200.0;
const VALUE_RULER_WIDTH: f32 = 52.0;
const HANDLE_LENGTH: f32 = 40.0;
const MIN_FRAME_SPAN: f32 = 0.5;
const MIN_VALUE_SPAN: f32 = 1e-4;
const MAX_SPAN: f32 = 1e7;

const TRACK_PALETTE: [egui::Color32; 8] = [
    egui::Color32::from_rgb(224, 96, 96),
    egui::Color32::from_rgb(96, 170, 224),
    egui::Color32::from_rgb(120, 200, 120),
    egui::Color32::from_rgb(230, 190, 90),
    egui::Color32::from_rgb(190, 120, 224),
    egui::Color32::from_rgb(230, 140, 90),
    egui::Color32::from_rgb(90, 210, 200),
    egui::Color32::from_rgb(224, 120, 170),
];

pub fn track_color(index: usize) -> egui::Color32 {
    TRACK_PALETTE[index % TRACK_PALETTE.len()]
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimelineTrack {
    pub label: String,
    pub content_idx: usize,
    pub info_idx: usize,
    pub target_idx: usize,
}

#[derive(Clone, Debug)]
pub enum TimelineRow {
    PaneHeader { content_idx: usize, label: String },
    Track(TimelineTrack),
}

impl TimelineRow {
    pub const HEIGHT: f32 = 22.0;

    pub fn build(anim: &AnimInstance, expanded: &HashSet<usize>) -> Vec<Self> {
        let mut out = Vec::new();

        for (content_idx, content) in anim.bflan.anim_info.contents.iter().enumerate() {
            let target_count: usize = content
                .infos
                .iter()
                .map(|info| match info {
                    AnimInfo::Standard { targets, .. } => targets.len(),
                    _ => 0,
                })
                .sum();

            if target_count == 0 {
                continue;
            }

            out.push(Self::PaneHeader {
                content_idx,
                label: content.name.trim_end_matches('\0').to_string(),
            });

            if !expanded.contains(&content_idx) {
                continue;
            }

            for (info_idx, info) in content.infos.iter().enumerate() {
                if let AnimInfo::Standard { targets, .. } = info {
                    for (target_idx, target) in targets.iter().enumerate() {
                        out.push(Self::Track(TimelineTrack {
                            label: format!("{:?}", target.target),
                            content_idx,
                            info_idx,
                            target_idx,
                        }));
                    }
                }
            }
        }

        out
    }
}

fn all_tracks(anim: &AnimInstance) -> Vec<TimelineTrack> {
    let every = (0..anim.bflan.anim_info.contents.len()).collect();

    TimelineRow::build(anim, &every)
        .into_iter()
        .filter_map(|row| match row {
            TimelineRow::Track(track) => Some(track),
            TimelineRow::PaneHeader { .. } => None,
        })
        .collect()
}

fn key_count(curve: &Curve) -> usize {
    match curve {
        Curve::Constant(values) => values.len(),
        Curve::Step(keys) => keys.len(),
        Curve::Hermite(keys) => keys.len(),
    }
}

fn curve_bounds(curve: &Curve) -> Option<(f32, f32, f32, f32)> {
    let count = key_count(curve);

    if count == 0 {
        return None;
    }

    let first = key_point(curve, 0).0;
    let last = key_point(curve, count - 1).0;
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);

    for idx in 0..count {
        let value = key_point(curve, idx).1;

        lo = lo.min(value);
        hi = hi.max(value);
    }

    if matches!(curve, Curve::Hermite(_)) && last > first {
        for step in 0..=128 {
            let value = eval_curve(curve, first + (last - first) * step as f32 / 128.0);
            lo = lo.min(value);
            hi = hi.max(value);
        }
    }

    Some((first, last, lo, hi))
}

#[derive(Clone, Copy, Debug)]
struct Norm {
    mid: f32,
    scale: f32,
}

impl Norm {
    const IDENTITY: Self = Self {
        mid: 0.0,
        scale: 1.0,
    };

    fn of(curve: &Curve) -> Self {
        match curve_bounds(curve) {
            Some((_, _, lo, hi)) if hi - lo > f32::EPSILON => Self {
                mid: (lo + hi) * 0.5,
                scale: 2.0 / (hi - lo),
            },
            Some((_, _, lo, _)) => Self {
                mid: lo,
                scale: 1.0,
            },
            None => Self::IDENTITY,
        }
    }

    fn show(self, value: f32) -> f32 {
        (value - self.mid) * self.scale
    }

    fn real(self, shown: f32) -> f32 {
        shown / self.scale + self.mid
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphView {
    pub frame_min: f32,
    pub frame_max: f32,
    pub value_min: f32,
    pub value_max: f32,
}

impl GraphView {
    fn fit<'a>(
        curves: impl IntoIterator<Item = (&'a Curve, Norm)>,
        frame_count: f32,
        whole_animation: bool,
    ) -> Self {
        let (mut f0, mut f1) = if whole_animation {
            (0.0, frame_count)
        } else {
            (f32::INFINITY, f32::NEG_INFINITY)
        };
        let (mut v0, mut v1) = (f32::INFINITY, f32::NEG_INFINITY);

        for (curve, norm) in curves {
            if let Some((first, last, lo, hi)) = curve_bounds(curve) {
                f0 = f0.min(first);
                f1 = f1.max(last);
                v0 = v0.min(norm.show(lo));
                v1 = v1.max(norm.show(hi));
            }
        }

        if !f0.is_finite() {
            (f0, f1) = (0.0, frame_count);
        }

        if f1 - f0 < 1.0 {
            let mid = (f0 + f1) * 0.5;

            (f0, f1) = (mid - 5.0, mid + 5.0);
        }

        if !v0.is_finite() {
            (v0, v1) = (-1.0, 1.0);
        } else if v1 - v0 < MIN_VALUE_SPAN {
            let mid = (v0 + v1) * 0.5;
            let reach = (mid.abs() * 0.1).max(1.0);
            (v0, v1) = (mid - reach, mid + reach);
        } else {
            let pad = (v1 - v0) * 0.1;
            (v0, v1) = (v0 - pad, v1 + pad);
        }

        let pad = (f1 - f0) * 0.03;

        Self {
            frame_min: f0 - pad,
            frame_max: f1 + pad,
            value_min: v0,
            value_max: v1,
        }
    }

    fn zoom_frames(&mut self, factor: f32, anchor: f32) {
        let span = self.frame_max - self.frame_min;
        let new_span = (span * factor).clamp(MIN_FRAME_SPAN, MAX_SPAN);
        let t = (anchor - self.frame_min) / span;

        self.frame_min = anchor - t * new_span;
        self.frame_max = self.frame_min + new_span;
    }

    fn zoom_values(&mut self, factor: f32, anchor: f32) {
        let span = self.value_max - self.value_min;
        let new_span = (span * factor).clamp(MIN_VALUE_SPAN, MAX_SPAN);
        let t = (anchor - self.value_min) / span;

        self.value_min = anchor - t * new_span;
        self.value_max = self.value_min + new_span;
    }

    fn pan(&mut self, frames: f32, values: f32) {
        self.frame_min += frames;
        self.frame_max += frames;
        self.value_min += values;
        self.value_max += values;
    }
}

#[derive(Clone, Copy)]
struct Mapper {
    rect: egui::Rect,
    view: GraphView,
}

impl Mapper {
    fn px_per_frame(&self) -> f32 {
        self.rect.width() / (self.view.frame_max - self.view.frame_min)
    }

    fn px_per_value(&self) -> f32 {
        self.rect.height() / (self.view.value_max - self.view.value_min)
    }

    fn x(&self, frame: f32) -> f32 {
        self.rect.min.x + (frame - self.view.frame_min) * self.px_per_frame()
    }

    fn y(&self, value: f32) -> f32 {
        self.rect.max.y - (value - self.view.value_min) * self.px_per_value()
    }

    fn pos(&self, frame: f32, value: f32) -> egui::Pos2 {
        egui::pos2(self.x(frame), self.y(value))
    }

    fn frame_at(&self, x: f32) -> f32 {
        self.view.frame_min + (x - self.rect.min.x) / self.px_per_frame()
    }

    fn value_at(&self, y: f32) -> f32 {
        self.view.value_min + (self.rect.max.y - y) / self.px_per_value()
    }
}

fn tick_step(span: f32, px: f32, min_px: f32, min_step: f32) -> f32 {
    let raw = (span * min_px / px.max(1.0)).max(min_step);
    let base = 10f32.powf(raw.log10().floor());

    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * base)
        .find(|&step| step >= raw)
        .unwrap_or(10.0 * base)
}

fn ticks(lo: f32, hi: f32, step: f32) -> impl Iterator<Item = f32> {
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;

    (first..=last).map(move |i| i as f32 * step)
}

fn format_tick(value: f32, step: f32) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;

    let value = if value.abs() < step * 1e-3 {
        0.0
    } else {
        value
    };

    format!("{value:.decimals$}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    All,
    Selected,
}

struct GraphRects {
    graph: egui::Rect,
    ruler: egui::Rect,
    values: egui::Rect,
}

#[derive(Clone, Debug)]
pub struct PendingKeyEdit {
    pub track: TimelineTrack,
    pub key_idx: usize,
    pub frame: f32,
    pub value: f32,
}

#[derive(Clone, Debug)]
pub struct PendingSlopeEdit {
    pub track: TimelineTrack,
    pub key_idx: usize,
    pub slope: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedKey {
    pub track: TimelineTrack,
    pub key_idx: usize,
}

pub struct TimelineState {
    pub anim_player: AnimPlayer,
    pub pending_key_edit: Option<PendingKeyEdit>,
    pub pending_slope_edit: Option<PendingSlopeEdit>,
    pub selected_key: Option<SelectedKey>,
    pub selected_track: Option<TimelineTrack>,
    pub expanded_anim_panes: HashSet<usize>,
    pub hidden_tracks: HashSet<TimelineTrack>,
    pub views: HashMap<usize, GraphView>,
    pub normalize: bool,
    pub panning: bool,
    pub frame_rate: f32,

    pub pending_anim_edit: Option<(String, PaneAnimInfo)>,
    pub pending_frame_count: Option<u16>,
    pub animatable: Vec<Animatable>,
    pub animatable_key: Option<(u64, usize)>,
    pub edit_error: Option<String>,
    texture_draft: String,
    dragging: bool,
    norms: HashMap<TimelineTrack, Norm>,
    fit: Option<Fit>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            anim_player: AnimPlayer::new(),
            pending_key_edit: None,
            pending_slope_edit: None,
            selected_key: None,
            selected_track: None,
            hidden_tracks: HashSet::new(),
            expanded_anim_panes: HashSet::new(),
            views: HashMap::new(),
            normalize: false,
            panning: false,
            frame_rate: 30.0,
            pending_anim_edit: None,
            pending_frame_count: None,
            animatable: Vec::new(),
            animatable_key: None,
            edit_error: None,
            texture_draft: String::new(),
            dragging: false,
            norms: HashMap::new(),
            fit: None,
        }
    }
}

impl TimelineState {
    pub fn reset_tracks(&mut self) {
        self.selected_key = None;
        self.selected_track = None;
        self.pending_key_edit = None;
        self.pending_slope_edit = None;
        self.hidden_tracks.clear();
        self.expanded_anim_panes.clear();
        self.norms.clear();
    }

    fn propose(
        &mut self,
        active_idx: usize,
        label: String,
        edit: impl FnOnce(&mut PaneAnimInfo) -> Result<(), String>,
    ) {
        let Some(anim) = self.anim_player.anims.get(active_idx) else {
            return;
        };

        let mut info = anim.bflan.anim_info.clone();

        match edit(&mut info) {
            Ok(()) => {
                self.pending_anim_edit = Some((label, info));
                self.edit_error = None;
            }
            Err(err) => self.edit_error = Some(err),
        }
    }

    fn track_menu(&mut self, ui: &mut egui::Ui, active_idx: usize, target: &Animatable) {
        for kind in anim_edit::kinds_for(&target.anim_type) {
            ui.menu_button(kind.label, |ui| {
                let layers = if kind.layered {
                    target.layers.max(1)
                } else {
                    1
                };

                for layer in 0..layers {
                    let mut targets = |ui: &mut egui::Ui| {
                        for raw in 0..kind.targets {
                            let name = anim_edit::target_name(kind.info_type, raw);

                            if ui.button(&name).clicked() {
                                self.propose(
                                    active_idx,
                                    format!("Add {name} track to {}", target.name),
                                    |info| {
                                        anim_edit::add_track(info, target, kind, raw, layer as u8)
                                    },
                                );

                                ui.close();
                            }
                        }
                    };

                    if layers > 1 {
                        ui.menu_button(format!("Texture {}", layer + 1), targets);
                    } else {
                        targets(ui);
                    }
                }
            });
        }
    }

    fn animate_menu(&mut self, ui: &mut egui::Ui, active_idx: usize) {
        let animatable = self.animatable.clone();

        ui.menu_button("Animate…", |ui| {
            for (anim_type, title) in [(AnimType::Pane, "Pane"), (AnimType::Material, "Material")] {
                ui.menu_button(title, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for target in animatable.iter().filter(|t| t.anim_type == anim_type) {
                                ui.menu_button(&target.name, |ui| {
                                    self.track_menu(ui, active_idx, target)
                                });
                            }
                        });
                });
            }
        });
    }
}

impl DrawUi<()> for TimelineState {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let Some(active_idx) = self.anim_player.active else {
            return;
        };

        let (frame_count, info_frames, anim_name, anim_frame, rows, anim_groups, colors) = {
            let Some(anim) = self.anim_player.anims.get(active_idx) else {
                return;
            };

            let colors: HashMap<TimelineTrack, egui::Color32> = all_tracks(anim)
                .into_iter()
                .enumerate()
                .map(|(idx, track)| (track, track_color(idx)))
                .collect();

            (
                anim.frame_count().max(1.0),
                anim.bflan.anim_info.frame_count,
                anim.name.clone(),
                anim.frame,
                TimelineRow::build(anim, &self.expanded_anim_panes),
                anim.bflan.anim_tag.groups.clone(),
                colors,
            )
        };

        egui::Panel::bottom("timeline_panel")
            .resizable(true)
            .default_size(320.0)
            .min_size(120.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Timeline");
                    ui.separator();

                    ui.label(format!(
                        "{} - frame {:.1} / {frame_count:.0}",
                        anim_name, anim_frame
                    ));

                    ui.separator();

                    if !anim_groups.is_empty() {
                        ui.label("Groups:");
                        for group in anim_groups {
                            ui.label(group);
                        }
                    }

                    ui.separator();
                    ui.label("Total Frames:");

                    let mut frames = info_frames;

                    if ui
                        .add(egui::DragValue::new(&mut frames).range(1..=u16::MAX))
                        .changed()
                    {
                        self.pending_frame_count = Some(frames);
                    }

                    self.animate_menu(ui, active_idx);

                    if let Some(error) = &self.edit_error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::DragValue::new(&mut self.frame_rate).speed(1));
                        ui.label("FPS:");

                        ui.separator();

                        if ui.checkbox(&mut self.normalize, "Normalize").changed() {
                            self.norms.clear();
                            self.fit = Some(Fit::All);
                        }

                        if ui.small_button("Frame selected").clicked() {
                            self.fit = Some(Fit::Selected);
                        }

                        if ui.small_button("Frame all").clicked() {
                            self.fit = Some(Fit::All);
                        }
                    });
                });

                ui.separator();

                if rows.is_empty() {
                    ui.weak("No animated panes in this animation.");
                    return;
                }

                let content_rect = ui.available_rect_before_wrap();
                let inspector_w = if self.selected_key.is_some() {
                    INSPECTOR_WIDTH
                } else {
                    0.0
                };

                let top = content_rect.min.y + TIMELINE_RULER_HEIGHT;
                let graph_x0 = content_rect.min.x + LABEL_COL_WIDTH + VALUE_RULER_WIDTH;

                let graph = egui::Rect::from_min_max(
                    egui::pos2(graph_x0, top),
                    egui::pos2(content_rect.max.x - inspector_w, content_rect.max.y),
                );

                let rects = GraphRects {
                    graph,
                    ruler: egui::Rect::from_min_max(
                        egui::pos2(graph.min.x, content_rect.min.y),
                        egui::pos2(graph.max.x, top),
                    ),
                    values: egui::Rect::from_min_max(
                        egui::pos2(graph.min.x - VALUE_RULER_WIDTH, top),
                        egui::pos2(graph.min.x, graph.max.y),
                    ),
                };

                let list_rect = egui::Rect::from_min_max(
                    egui::pos2(content_rect.min.x, top),
                    egui::pos2(content_rect.min.x + LABEL_COL_WIDTH, content_rect.max.y),
                );

                self.draw_track_list(ui, active_idx, &rows, &colors, list_rect, anim_frame);

                let shown: Vec<TimelineTrack> = rows
                    .iter()
                    .filter_map(|row| match row {
                        TimelineRow::Track(track) if !self.hidden_tracks.contains(track) => {
                            Some(track.clone())
                        }
                        _ => None,
                    })
                    .collect();

                self.draw_graph(ui, active_idx, &shown, &colors, &rects);

                if self.selected_key.is_some() {
                    let inspector_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.max.x - INSPECTOR_WIDTH, content_rect.min.y),
                        content_rect.max,
                    );

                    self.draw_key_inspector(ui, active_idx, inspector_rect);
                }

                ui.advance_cursor_after_rect(content_rect);
            });
    }
}

impl TimelineState {
    fn draw_track_list(
        &mut self,
        ui: &mut egui::Ui,
        active_idx: usize,
        rows: &[TimelineRow],
        colors: &HashMap<TimelineTrack, egui::Color32>,
        rect: egui::Rect,
        anim_frame: f32,
    ) {
        let mut list_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );

        egui::ScrollArea::vertical()
            .id_salt("timeline_tracks")
            .auto_shrink([false, false])
            .show(&mut list_ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                for row in rows {
                    let (row_rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), TimelineRow::HEIGHT),
                        egui::Sense::click(),
                    );

                    let mid_y = row_rect.center().y;

                    match row {
                        TimelineRow::PaneHeader { content_idx, label } => {
                            let expanded = self.expanded_anim_panes.contains(content_idx);

                            if response.clicked() {
                                if expanded {
                                    self.expanded_anim_panes.remove(content_idx);
                                } else {
                                    self.expanded_anim_panes.insert(*content_idx);
                                }
                            }

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                ui.painter().rect_filled(
                                    row_rect,
                                    2.0,
                                    ui.visuals().widgets.hovered.weak_bg_fill,
                                );
                            }

                            self.header_menu(&response, active_idx, *content_idx, label);

                            let c = egui::pos2(row_rect.min.x + 10.0, mid_y);
                            let arrow = if expanded {
                                vec![
                                    c + egui::vec2(-4.0, -2.0),
                                    c + egui::vec2(4.0, -2.0),
                                    c + egui::vec2(0.0, 3.0),
                                ]
                            } else {
                                vec![
                                    c + egui::vec2(-2.0, -4.0),
                                    c + egui::vec2(3.0, 0.0),
                                    c + egui::vec2(-2.0, 4.0),
                                ]
                            };

                            ui.painter().add(egui::Shape::convex_polygon(
                                arrow,
                                egui::Color32::from_gray(200),
                                egui::Stroke::NONE,
                            ));

                            ui.painter().text(
                                egui::pos2(row_rect.min.x + 20.0, mid_y),
                                egui::Align2::LEFT_CENTER,
                                label,
                                egui::FontId::proportional(12.0),
                                egui::Color32::WHITE,
                            );
                        }

                        TimelineRow::Track(track) => {
                            if self.selected_track.as_ref() == Some(track) {
                                ui.painter().rect_filled(
                                    row_rect,
                                    2.0,
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.5),
                                );
                            } else if response.hovered() {
                                ui.painter().rect_filled(
                                    row_rect,
                                    2.0,
                                    ui.visuals().widgets.hovered.weak_bg_fill,
                                );
                            }

                            if response.clicked() {
                                self.selected_track = Some(track.clone());
                            }

                            self.track_context_menu(&response, active_idx, track, anim_frame);

                            let mut visible = !self.hidden_tracks.contains(track);

                            let checkbox_rect = egui::Rect::from_min_size(
                                egui::pos2(row_rect.min.x + 16.0, mid_y - 8.0),
                                egui::vec2(16.0, 16.0),
                            );

                            let mut checkbox_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(checkbox_rect)
                                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            );

                            if checkbox_ui.checkbox(&mut visible, "").changed() {
                                if visible {
                                    self.hidden_tracks.remove(track);
                                } else {
                                    self.hidden_tracks.insert(track.clone());
                                }
                            }

                            let color = colors.get(track).copied().unwrap_or(egui::Color32::GRAY);

                            ui.painter().rect_filled(
                                egui::Rect::from_center_size(
                                    egui::pos2(row_rect.min.x + 44.0, mid_y),
                                    egui::vec2(8.0, 8.0),
                                ),
                                1.0,
                                color,
                            );

                            ui.painter().text(
                                egui::pos2(row_rect.min.x + 54.0, mid_y),
                                egui::Align2::LEFT_CENTER,
                                &track.label,
                                egui::FontId::proportional(11.0),
                                if visible {
                                    egui::Color32::from_gray(200)
                                } else {
                                    egui::Color32::from_gray(110)
                                },
                            );
                        }
                    }
                }
            });
    }

    fn header_menu(
        &mut self,
        response: &egui::Response,
        active_idx: usize,
        content_idx: usize,
        label: &str,
    ) {
        response.context_menu(|ui| {
            let anim_type = self
                .anim_player
                .anims
                .get(active_idx)
                .and_then(|a| a.bflan.anim_info.contents.get(content_idx))
                .map(|c| c.anim_type.clone());

            let target = self
                .animatable
                .iter()
                .find(|t| Some(&t.anim_type) == anim_type.as_ref() && t.name == label)
                .cloned();

            if let Some(target) = target {
                ui.menu_button("Add track", |ui| self.track_menu(ui, active_idx, &target));
            }

            if ui.button("Remove from animation").clicked() {
                self.propose(
                    active_idx,
                    format!("Remove {label} from the animation"),
                    |info| {
                        anim_edit::remove_content(info, content_idx)
                            .ok_or_else(|| "It is no longer animated".into())
                    },
                );

                ui.close();
            }
        });
    }

    fn track_context_menu(
        &mut self,
        response: &egui::Response,
        active_idx: usize,
        track: &TimelineTrack,
        anim_frame: f32,
    ) {
        response.context_menu(|ui| {
            let frame = anim_frame.round();

            if ui.button(format!("Add key at frame {frame}")).clicked() {
                self.propose(active_idx, format!("Add key to {}", track.label), |info| {
                    anim_edit::add_key(info, track, frame).map(|_| ())
                });

                ui.close();
            }

            if ui.button("Delete track").clicked() {
                self.propose(
                    active_idx,
                    format!("Delete track {}", track.label),
                    |info| {
                        anim_edit::delete_track(info, track)
                            .ok_or_else(|| "The track no longer exists".into())
                    },
                );

                ui.close();
            }
        });
    }

    pub fn frame_view(&mut self) {
        let selected = self.selected_key.is_some() || self.selected_track.is_some();
        self.fit = Some(if selected { Fit::Selected } else { Fit::All });
    }

    fn norm(&mut self, track: &TimelineTrack, curve: &Curve) -> Norm {
        if self.normalize {
            *self
                .norms
                .entry(track.clone())
                .or_insert_with(|| Norm::of(curve))
        } else {
            Norm::IDENTITY
        }
    }

    fn update_view(
        &mut self,
        ui: &egui::Ui,
        active_idx: usize,
        curves: &[(TimelineTrack, Curve, Norm)],
        rects: &GraphRects,
    ) -> GraphView {
        let Some(anim) = self.anim_player.anims.get(active_idx) else {
            return GraphView::fit([], 1.0, true);
        };

        let frame_count = anim.frame_count().max(1.0);
        let hovered = ui.rect_contains_pointer(rects.graph);

        let fit = self.fit.take();

        if fit.is_some() || !self.views.contains_key(&active_idx) {
            let selected = self
                .selected_key
                .as_ref()
                .map(|s| &s.track)
                .or(self.selected_track.as_ref());

            let view = match fit {
                Some(Fit::Selected) if let Some(selected) = selected => {
                    let picked = curves.iter().filter(|(t, ..)| t == selected);
                    GraphView::fit(picked.map(|(_, c, n)| (c, *n)), frame_count, false)
                }
                _ if !curves.is_empty() || fit.is_some() => {
                    GraphView::fit(curves.iter().map(|(_, c, n)| (c, *n)), frame_count, true)
                }
                _ => {
                    let every: Vec<Curve> = all_tracks(anim)
                        .iter()
                        .filter_map(|t| anim.curve(t).cloned())
                        .collect();

                    let normalize = self.normalize;

                    GraphView::fit(
                        every.iter().map(|c| {
                            let norm = if normalize {
                                Norm::of(c)
                            } else {
                                Norm::IDENTITY
                            };
                            (c, norm)
                        }),
                        frame_count,
                        true,
                    )
                }
            };

            self.views.insert(active_idx, view);
        }

        let mut view = self.views[&active_idx];
        let mapper = Mapper {
            rect: rects.graph,
            view,
        };

        if hovered && let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));

            if scroll.y.abs() > f32::EPSILON {
                view.zoom_frames((-scroll.y * 0.002).exp(), mapper.frame_at(pointer.x));
            }

            if scroll.x.abs() > f32::EPSILON {
                view.pan(-scroll.x / mapper.px_per_frame(), 0.0);
            }

            if (zoom - 1.0).abs() > f32::EPSILON {
                view.zoom_values(1.0 / zoom, mapper.value_at(pointer.y));
            }
        }

        view
    }

    fn draw_graph(
        &mut self,
        ui: &mut egui::Ui,
        active_idx: usize,
        shown: &[TimelineTrack],
        colors: &HashMap<TimelineTrack, egui::Color32>,
        rects: &GraphRects,
    ) {
        let Some(anim) = self.anim_player.anims.get(active_idx) else {
            return;
        };

        let frame_count = anim.frame_count().max(1.0);
        let anim_frame = anim.frame;

        let curves: Vec<(TimelineTrack, Curve)> = shown
            .iter()
            .filter_map(|t| anim.curve(t).map(|c| (t.clone(), c.clone())))
            .collect();

        if !self.dragging {
            self.norms.clear();
        }

        let curves: Vec<(TimelineTrack, Curve, Norm)> = curves
            .into_iter()
            .map(|(t, c)| {
                let norm = self.norm(&t, &c);
                (t, c, norm)
            })
            .collect();

        let canvas = ui.interact(
            rects.graph,
            ui.id().with("timeline_graph"),
            egui::Sense::click_and_drag(),
        );

        let ruler = ui.interact(
            rects.ruler,
            ui.id().with("timeline_ruler"),
            egui::Sense::click_and_drag(),
        );

        let mut view = self.update_view(ui, active_idx, &curves, rects);

        if canvas.drag_started() && !self.dragging {
            self.panning = true;
        }

        if self.panning && canvas.dragged() {
            let mapper = Mapper {
                rect: rects.graph,
                view,
            };

            let delta = canvas.drag_delta();
            view.pan(
                -delta.x / mapper.px_per_frame(),
                delta.y / mapper.px_per_value(),
            );
        }

        if canvas.drag_stopped() {
            self.panning = false;
        }

        if canvas.clicked() {
            self.selected_key = None;
        }

        self.views.insert(active_idx, view);

        let mapper = Mapper {
            rect: rects.graph,
            view,
        };

        if (ruler.clicked() || ruler.dragged())
            && let Some(pos) = ruler.interact_pointer_pos()
            && let Some(anim) = self.anim_player.anims.get_mut(active_idx)
        {
            anim.frame = mapper.frame_at(pos.x).round().clamp(0.0, frame_count);
            anim.playing = false;
            self.anim_player.preview_enabled = true;
        }

        if ruler.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        draw_grid(ui, &mapper, rects, frame_count, self.normalize);

        let painter = ui.painter_at(rects.graph);

        if curves.is_empty() {
            painter.text(
                rects.graph.center(),
                egui::Align2::CENTER_CENTER,
                "Expand a pane to edit its curves",
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(120),
            );
        }

        let mut any_point_active = false;
        let frames = (view.frame_min, view.frame_max);

        for (track, curve, norm) in &curves {
            let norm = *norm;
            let color = colors.get(track).copied().unwrap_or(egui::Color32::GRAY);
            let is_track_selected = self.selected_track.as_ref() == Some(track);
            let width = if is_track_selected { 2.5 } else { 1.5 };

            draw_curve(
                &painter,
                &|f, v| mapper.pos(f, norm.show(v)),
                curve,
                egui::Stroke::new(width, color),
                frames,
            );

            for key_idx in 0..key_count(curve) {
                let (frame, value) = key_point(curve, key_idx);
                let screen_pos = mapper.pos(frame, norm.show(value));

                if !rects
                    .graph
                    .expand(TIMELINE_MARKER_RADIUS)
                    .contains(screen_pos)
                {
                    continue;
                }

                let point_rect = egui::Rect::from_center_size(
                    screen_pos,
                    egui::Vec2::splat(TIMELINE_MARKER_RADIUS * 2.5),
                );

                let point_id = ui.id().with((
                    "timeline_key",
                    track.content_idx,
                    track.info_idx,
                    track.target_idx,
                    key_idx,
                ));

                let point_response =
                    ui.interact(point_rect, point_id, egui::Sense::click_and_drag());

                if point_response.drag_started() || point_response.clicked() {
                    self.selected_key = Some(SelectedKey {
                        track: track.clone(),
                        key_idx,
                    });

                    self.selected_track = Some(track.clone());
                }

                if point_response.dragged() {
                    any_point_active = true;

                    if let Some(pos) = point_response.interact_pointer_pos() {
                        self.pending_key_edit = Some(PendingKeyEdit {
                            track: track.clone(),
                            key_idx,
                            frame: mapper.frame_at(pos.x),
                            value: norm.real(mapper.value_at(pos.y)),
                        });
                    }
                }

                if point_response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    point_response.on_hover_text_at_pointer(format!("{frame:.2}, {value:.3}"));
                }

                let is_selected = self
                    .selected_key
                    .as_ref()
                    .is_some_and(|s| s.track == *track && s.key_idx == key_idx);

                painter.circle_filled(screen_pos, TIMELINE_MARKER_RADIUS, color);
                painter.circle_stroke(
                    screen_pos,
                    TIMELINE_MARKER_RADIUS,
                    egui::Stroke::new(
                        if is_selected { 2.0 } else { 1.0 },
                        if is_selected {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        },
                    ),
                );
            }

            if let Curve::Hermite(keys) = curve
                && let Some(selected) = &self.selected_key
                && selected.track == *track
                && let Some(key) = keys.get(selected.key_idx)
            {
                let key_screen = mapper.pos(key.frame, norm.show(key.value));
                let slope_px =
                    -key.slope * norm.scale * mapper.px_per_value() / mapper.px_per_frame();
                let dir = egui::vec2(1.0, slope_px).normalized() * HANDLE_LENGTH;
                let key_idx = selected.key_idx;

                for (side, handle_screen) in [(0, key_screen + dir), (1, key_screen - dir)] {
                    painter.line_segment(
                        [key_screen, handle_screen],
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    );

                    let handle_rect = egui::Rect::from_center_size(
                        handle_screen,
                        egui::Vec2::splat(TIMELINE_MARKER_RADIUS * 2.0),
                    );

                    let handle_id = ui.id().with((
                        "timeline_handle",
                        track.content_idx,
                        track.info_idx,
                        track.target_idx,
                        key_idx,
                        side,
                    ));

                    let handle_response =
                        ui.interact(handle_rect, handle_id, egui::Sense::click_and_drag());

                    if handle_response.dragged() {
                        any_point_active = true;

                        if let Some(pos) = handle_response.interact_pointer_pos() {
                            let delta = pos - key_screen;
                            let df = delta.x / mapper.px_per_frame();

                            if df.abs() > 1e-4 {
                                let dv = -delta.y / mapper.px_per_value();

                                self.pending_slope_edit = Some(PendingSlopeEdit {
                                    track: track.clone(),
                                    key_idx,
                                    slope: dv / df / norm.scale,
                                });
                            }
                        }
                    }

                    if handle_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    painter.add(egui::Shape::convex_polygon(
                        diamond_points(handle_screen, TIMELINE_MARKER_RADIUS * 0.9),
                        egui::Color32::WHITE,
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                    ));
                }
            }
        }

        let playhead_x = mapper.x(anim_frame);
        let playhead = egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 90));

        painter.vline(playhead_x, rects.graph.y_range(), playhead);
        ui.painter_at(rects.ruler)
            .vline(playhead_x, rects.ruler.y_range(), playhead);

        if self.panning || any_point_active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        self.dragging = any_point_active;
    }

    fn draw_key_inspector(&mut self, ui: &mut egui::Ui, active_idx: usize, rect: egui::Rect) {
        let Some(anim) = self.anim_player.anims.get(active_idx) else {
            return;
        };

        let Some(selected) = self.selected_key.clone() else {
            return;
        };

        let Some(curve) = anim.curve(&selected.track) else {
            self.selected_key = None;
            return;
        };

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let ui = &mut child;

        ui.separator();
        ui.heading("Keyframe");
        ui.label(egui::RichText::new(&selected.track.label).weak());
        ui.add_space(6.0);

        match curve {
            Curve::Hermite(keys) => {
                let Some(key) = keys.get(selected.key_idx) else {
                    return;
                };
                let (mut frame, mut value, mut slope) = (key.frame, key.value, key.slope);

                egui::Grid::new("timeline_inspector_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Frame");
                        if ui
                            .add(egui::DragValue::new(&mut frame).speed(0.1))
                            .changed()
                        {
                            self.pending_key_edit = Some(PendingKeyEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                frame,
                                value,
                            });
                        }
                        ui.end_row();

                        ui.label("Value");
                        if ui
                            .add(egui::DragValue::new(&mut value).speed(0.1))
                            .changed()
                        {
                            self.pending_key_edit = Some(PendingKeyEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                frame,
                                value,
                            });
                        }
                        ui.end_row();

                        ui.label("Slope");
                        if ui
                            .add(egui::DragValue::new(&mut slope).speed(0.01))
                            .changed()
                        {
                            self.pending_slope_edit = Some(PendingSlopeEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                slope,
                            });
                        }
                        ui.end_row();
                    });
            }

            Curve::Step(keys) => {
                let Some(key) = keys.get(selected.key_idx) else {
                    return;
                };
                let (mut frame, mut value) = (key.frame, key.value as f32);

                egui::Grid::new("timeline_inspector_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Frame");
                        if ui
                            .add(egui::DragValue::new(&mut frame).speed(0.1))
                            .changed()
                        {
                            self.pending_key_edit = Some(PendingKeyEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                frame,
                                value,
                            });
                        }
                        ui.end_row();

                        ui.label("Value");
                        if ui
                            .add(
                                egui::DragValue::new(&mut value)
                                    .speed(1.0)
                                    .range(0.0..=65535.0),
                            )
                            .changed()
                        {
                            self.pending_key_edit = Some(PendingKeyEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                frame,
                                value,
                            });
                        }
                        ui.end_row();
                    });
            }

            Curve::Constant(values) => {
                let Some(&v) = values.get(selected.key_idx) else {
                    return;
                };
                let mut value = v;

                ui.horizontal(|ui| {
                    ui.label("Value");
                    if ui
                        .add(egui::DragValue::new(&mut value).speed(1.0))
                        .changed()
                    {
                        self.pending_key_edit = Some(PendingKeyEdit {
                            track: selected.track.clone(),
                            key_idx: selected.key_idx,
                            frame: selected.key_idx as f32,
                            value,
                        });
                    }
                });
            }
        }

        if anim_edit::track_info_type(&anim.bflan.anim_info, &selected.track)
            == Some(AnimInfoType::TexturePatternAnim)
            && let Curve::Step(keys) = curve
            && let Some(key) = keys.get(selected.key_idx)
        {
            let textures = &anim.bflan.anim_info.textures;
            let current = textures
                .get(key.value as usize)
                .cloned()
                .unwrap_or_else(|| format!("#{} (missing)", key.value));

            ui.add_space(6.0);

            egui::ComboBox::from_id_salt("timeline_pattern_texture")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (index, texture) in textures.iter().enumerate() {
                        if ui
                            .selectable_label(index == key.value as usize, texture)
                            .clicked()
                        {
                            self.pending_key_edit = Some(PendingKeyEdit {
                                track: selected.track.clone(),
                                key_idx: selected.key_idx,
                                frame: key.frame,
                                value: index as f32,
                            });
                        }
                    }
                });

            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.texture_draft).desired_width(110.0));
                let name = self.texture_draft.trim().to_string();

                if ui
                    .add_enabled(!name.is_empty(), egui::Button::new("Add name"))
                    .clicked()
                {
                    if textures.contains(&name) {
                        self.edit_error = Some(format!("{name} is already in the list"));
                    } else {
                        let mut info = anim.bflan.anim_info.clone();
                        info.textures.push(name.clone());
                        self.pending_anim_edit = Some((format!("Add texture name {name}"), info));
                    }

                    self.texture_draft.clear();
                }
            });
        }

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.button("Delete key").clicked() {
                let mut info = anim.bflan.anim_info.clone();

                match anim_edit::delete_key(&mut info, &selected.track, selected.key_idx) {
                    Ok(()) => {
                        self.pending_anim_edit =
                            Some((format!("Delete key in {}", selected.track.label), info));
                        self.selected_key = None;
                    }
                    Err(err) => self.edit_error = Some(err),
                }
            }

            if ui.button("Close").clicked() {
                self.selected_key = None;
            }
        });
    }
}

fn diamond_points(center: egui::Pos2, r: f32) -> Vec<egui::Pos2> {
    vec![
        egui::pos2(center.x, center.y - r),
        egui::pos2(center.x + r, center.y),
        egui::pos2(center.x, center.y + r),
        egui::pos2(center.x - r, center.y),
    ]
}

fn key_point(curve: &Curve, idx: usize) -> (f32, f32) {
    match curve {
        Curve::Constant(keys) => (idx as f32, keys[idx]),
        Curve::Step(keys) => (keys[idx].frame, keys[idx].value as f32),
        Curve::Hermite(keys) => (keys[idx].frame, keys[idx].value),
    }
}

fn draw_grid(
    ui: &egui::Ui,
    mapper: &Mapper,
    rects: &GraphRects,
    frame_count: f32,
    normalize: bool,
) {
    let view = mapper.view;
    let graph = rects.graph;
    let painter = ui.painter_at(graph);
    let grid = egui::Stroke::new(1.0, egui::Color32::from_gray(48));
    let axis = egui::Stroke::new(1.0, egui::Color32::from_gray(90));
    let label_color = egui::Color32::from_gray(160);
    let font = egui::FontId::proportional(9.0);

    painter.rect_filled(graph, 0.0, egui::Color32::from_black_alpha(40));

    let outside = egui::Color32::from_black_alpha(70);
    let start_x = mapper.x(0.0);
    let end_x = mapper.x(frame_count);

    for (lo, hi) in [(graph.min.x, start_x), (end_x, graph.max.x)] {
        if hi > lo {
            painter.rect_filled(
                egui::Rect::from_x_y_ranges(lo..=hi, graph.y_range()),
                0.0,
                outside,
            );
        }
    }

    let frame_step = tick_step(view.frame_max - view.frame_min, graph.width(), 60.0, 1.0);

    let ruler = ui.painter_at(rects.ruler);

    for frame in ticks(view.frame_min, view.frame_max, frame_step) {
        let x = mapper.x(frame);
        painter.vline(x, graph.y_range(), grid);

        ruler.vline(x, rects.ruler.max.y - 6.0..=rects.ruler.max.y, axis);
        ruler.text(
            egui::pos2(x + 2.0, rects.ruler.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format_tick(frame, frame_step),
            font.clone(),
            label_color,
        );
    }

    ruler.hline(rects.ruler.x_range(), rects.ruler.max.y, axis);

    let values = ui.painter_at(rects.values);

    if normalize {
        for value in [-1.0, 0.0, 1.0] {
            painter.hline(
                graph.x_range(),
                mapper.y(value),
                if value == 0.0 { axis } else { grid },
            );
        }

        values.text(
            egui::pos2(rects.values.max.x - 4.0, mapper.y(0.0)),
            egui::Align2::RIGHT_CENTER,
            "norm",
            font,
            label_color,
        );
    } else {
        let value_step = tick_step(
            view.value_max - view.value_min,
            graph.height(),
            28.0,
            MIN_VALUE_SPAN * 0.1,
        );

        for value in ticks(view.value_min, view.value_max, value_step) {
            let y = mapper.y(value);

            let stroke = if value.abs() < value_step * 1e-3 {
                axis
            } else {
                grid
            };

            painter.hline(graph.x_range(), y, stroke);

            values.hline(rects.values.max.x - 4.0..=rects.values.max.x, y, axis);
            values.text(
                egui::pos2(rects.values.max.x - 6.0, y),
                egui::Align2::RIGHT_CENTER,
                format_tick(value, value_step),
                font.clone(),
                label_color,
            );
        }
    }

    values.vline(rects.values.max.x, rects.values.y_range(), axis);
}

fn draw_curve(
    painter: &egui::Painter,
    map: &dyn Fn(f32, f32) -> egui::Pos2,
    curve: &Curve,
    stroke: egui::Stroke,
    (view_start, view_end): (f32, f32),
) {
    let hold = |from: f32, to: f32, value: f32| {
        if to > from {
            painter.line_segment([map(from, value), map(to, value)], stroke);
        }
    };

    match curve {
        Curve::Hermite(keys) => {
            let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
                return;
            };

            hold(view_start, first.frame, first.value);

            for pair in keys.windows(2) {
                let [k0, k1] = pair else { continue };
                let dt = k1.frame - k0.frame;

                if dt.abs() < f32::EPSILON || k1.frame < view_start || k0.frame > view_end {
                    continue;
                }

                let points = [
                    map(k0.frame, k0.value),
                    map(k0.frame + dt / 3.0, k0.value + dt * k0.slope / 3.0),
                    map(k1.frame - dt / 3.0, k1.value - dt * k1.slope / 3.0),
                    map(k1.frame, k1.value),
                ];

                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    points,
                    false,
                    egui::Color32::TRANSPARENT,
                    stroke,
                ));
            }

            hold(last.frame, view_end, last.value);
        }

        Curve::Step(keys) => {
            let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
                return;
            };

            let mut points = vec![map(view_start.min(first.frame), first.value as f32)];

            for pair in keys.windows(2) {
                let [k0, k1] = pair else { continue };
                points.push(map(k1.frame, k0.value as f32));
                points.push(map(k1.frame, k1.value as f32));
            }

            points.push(map(view_end.max(last.frame), last.value as f32));
            painter.add(egui::epaint::PathShape::line(points, stroke));
        }

        Curve::Constant(values) => {
            let (Some(&first), Some(&last)) = (values.first(), values.last()) else {
                return;
            };

            let mut points = vec![map(view_start.min(0.0), first)];

            for (i, &v) in values.iter().enumerate() {
                points.push(map(i as f32, v));
                points.push(map((i + 1) as f32, v));
            }

            points.push(map(view_end.max(values.len() as f32), last));
            painter.add(egui::epaint::PathShape::line(points, stroke));
        }
    }
}
