use egui::{Ui, emath::Numeric};
use nnbfl::ui2d::types::{Color4f, Color4u8, Vector2f, Vector3f};

pub fn grid(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    body: impl FnOnce(&mut Ui) -> bool,
) -> bool {
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, body)
        .inner
}

pub fn section(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui) -> bool) -> bool {
    egui::CollapsingHeader::new(title)
        .default_open(open)
        .show(ui, body)
        .body_returned
        .unwrap_or(false)
}

pub fn row(ui: &mut Ui, label: &str, body: impl FnOnce(&mut Ui) -> bool) -> bool {
    ui.label(label);
    let changed = body(ui);
    ui.end_row();

    changed
}

pub fn drag<N: Numeric>(ui: &mut Ui, label: &str, value: &mut N, speed: f64) -> bool {
    row(ui, label, |ui| {
        ui.add(egui::DragValue::new(value).speed(speed)).changed()
    })
}

pub fn drag_in<N: Numeric>(
    ui: &mut Ui,
    label: &str,
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
) -> bool {
    row(ui, label, |ui| {
        ui.add(egui::DragValue::new(value).range(range)).changed()
    })
}

pub fn check(ui: &mut Ui, label: &str, value: &mut bool) -> bool {
    row(ui, label, |ui| ui.checkbox(value, "").changed())
}

pub fn text(ui: &mut Ui, label: &str, value: &mut String, limit: usize) -> bool {
    row(ui, label, |ui| {
        ui.add(egui::TextEdit::singleline(value).char_limit(limit))
            .changed()
    })
}

pub fn drags(ui: &mut Ui, values: &mut [&mut f32], names: &[&str], speed: f64) -> bool {
    ui.horizontal(|ui| {
        let mut changed = false;

        for (value, name) in values.iter_mut().zip(names) {
            ui.weak(*name);
            changed |= ui.add(egui::DragValue::new(*value).speed(speed)).changed();
        }

        changed
    })
    .inner
}

pub fn vec2(ui: &mut Ui, label: &str, v: &mut Vector2f, speed: f64) -> bool {
    row(ui, label, |ui| {
        drags(ui, &mut [&mut v.x, &mut v.y], &["X", "Y"], speed)
    })
}

pub fn vec3(ui: &mut Ui, label: &str, v: &mut Vector3f, speed: f64) -> bool {
    row(ui, label, |ui| {
        drags(
            ui,
            &mut [&mut v.x, &mut v.y, &mut v.z],
            &["X", "Y", "Z"],
            speed,
        )
    })
}

pub fn floats<const N: usize>(ui: &mut Ui, label: &str, v: &mut [f32; N], speed: f64) -> bool {
    row(ui, label, |ui| {
        let mut refs: Vec<&mut f32> = v.iter_mut().collect();
        drags(ui, &mut refs, &[""; N], speed)
    })
}

pub fn color(ui: &mut Ui, label: &str, c: &mut Color4u8) -> bool {
    row(ui, label, |ui| {
        let mut rgba = [c.r, c.g, c.b, c.a];
        let changed = ui.color_edit_button_srgba_unmultiplied(&mut rgba).changed();

        if changed {
            [c.r, c.g, c.b, c.a] = rgba;
        }

        changed
    })
}

pub fn color_f(ui: &mut Ui, label: &str, c: &mut Color4f) -> bool {
    row(ui, label, |ui| {
        let mut rgba: [f32; 4] = (*c).into();
        let changed = ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed();

        if changed {
            *c = rgba.into();
        }

        changed
    })
}

pub fn combo<T: Copy + Into<u8> + From<u8>>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    names: &[&str],
) -> bool {
    row(ui, label, |ui| {
        let current: u8 = (*value).into();
        let mut changed = false;

        egui::ComboBox::from_id_salt((label, ui.id()))
            .selected_text(names.get(current as usize).copied().unwrap_or("?"))
            .show_ui(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    if ui.selectable_label(i == current as usize, *name).clicked()
                        && i != current as usize
                    {
                        *value = T::from(i as u8);
                        changed = true;
                    }
                }
            });

        changed
    })
}

pub fn index_combo(ui: &mut Ui, label: &str, index: &mut u16, names: &[String]) -> bool {
    row(ui, label, |ui| {
        let mut changed = false;
        let current = names
            .get(*index as usize)
            .map_or_else(|| format!("#{index} (missing)"), |name| name.clone());

        egui::ComboBox::from_id_salt((label, ui.id()))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    if ui
                        .selectable_label(*index as usize == i, format!("[{}] {name}", i + 1))
                        .clicked()
                        && *index as usize != i
                    {
                        *index = i as u16;
                        changed = true;
                    }
                }
            });

        changed
    })
}

pub const HORIZONTAL: &[&str] = &["Center", "Left", "Right"];
pub const VERTICAL: &[&str] = &["Center", "Top", "Bottom"];
