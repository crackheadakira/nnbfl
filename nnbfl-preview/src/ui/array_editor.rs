use std::{fmt::Debug, hash::Hash};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayOp {
    Added(usize),
    Duplicated(usize),
    Removed(usize),
    Moved { from: usize, to: usize },
}

impl ArrayOp {
    fn apply<T: Clone + Default>(self, items: &mut Vec<T>) {
        match self {
            Self::Added(i) => items.insert(i, T::default()),
            Self::Duplicated(i) => items.insert(i + 1, items[i].clone()),
            Self::Removed(i) => {
                items.remove(i);
            }
            Self::Moved { from, to } => {
                let item = items.remove(from);
                items.insert(to, item);
            }
        }
    }

    pub fn mirror<T: Clone + Default>(self, items: &mut Vec<T>, grow: bool) {
        let fits = match self {
            Self::Added(i) => grow && i <= items.len(),
            Self::Duplicated(i) => grow && i < items.len(),
            Self::Removed(i) => i < items.len(),
            Self::Moved { from, to } => from < items.len() && to < items.len(),
        };

        if fits {
            self.apply(items);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ArrayResponse {
    pub changed: bool,
    pub op: Option<ArrayOp>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrayLayout {
    #[default]
    Rows,
    Cards,
    Inline,
}

type Label<'a, T> = Box<dyn Fn(usize, &T) -> String + 'a>;

pub struct ArrayEditor<'a, T> {
    id: egui::Id,
    items: &'a mut Vec<T>,
    title: Option<String>,
    max: usize,
    layout: ArrayLayout,
    addable: bool,
    duplicable: bool,
    label: Option<Label<'a, T>>,
}

impl<'a, T: Clone + Default> ArrayEditor<'a, T> {
    pub fn new(id_salt: impl Hash + Debug, items: &'a mut Vec<T>) -> Self {
        Self {
            id: egui::Id::new(id_salt),
            items,
            title: None,
            max: usize::MAX,
            layout: ArrayLayout::default(),
            addable: true,
            duplicable: true,
            label: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn max(mut self, max: usize) -> Self {
        self.max = max;
        self
    }

    pub fn layout(mut self, layout: ArrayLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn addable(mut self, addable: bool) -> Self {
        self.addable = addable;
        self
    }

    pub fn duplicable(mut self, duplicable: bool) -> Self {
        self.duplicable = duplicable;
        self
    }

    pub fn item_label(mut self, label: impl Fn(usize, &T) -> String + 'a) -> Self {
        self.label = Some(Box::new(label));
        self
    }

    pub fn show(
        self,
        ui: &mut egui::Ui,
        mut body: impl FnMut(&mut egui::Ui, usize, &mut T) -> bool,
    ) -> ArrayResponse {
        let Self {
            id,
            items,
            title,
            max,
            layout,
            addable,
            duplicable,
            label,
        } = self;

        let len = items.len();
        let can_add = addable && len < max;
        let can_duplicate = duplicable && len < max;
        let can_remove = len > 0;
        let label = |i: usize, item: &T| match &label {
            Some(label) => label(i, item),
            None => format!("#{}", i + 1),
        };

        let mut changed = false;
        let mut op = None;

        ui.push_id(id, |ui| {
            if layout != ArrayLayout::Inline {
                ui.horizontal(|ui| {
                    if let Some(title) = &title {
                        ui.heading(title);
                    }

                    if addable
                        && ui
                            .add_enabled(can_add, egui::Button::new("➕ Add Entry"))
                            .clicked()
                    {
                        op = Some(ArrayOp::Added(len));
                    }
                });
            }

            match layout {
                ArrayLayout::Rows => {
                    for (i, item) in items.iter_mut().enumerate() {
                        ui.push_id(i, |ui| {
                            ui.horizontal(|ui| {
                                ui.weak(label(i, item));
                                item_controls(ui, i, len, can_duplicate, can_remove, &mut op);
                            });

                            changed |= body(ui, i, item);
                            ui.add_space(8.0);
                        });
                    }
                }

                ArrayLayout::Cards => {
                    ui.horizontal_wrapped(|ui| {
                        for (i, item) in items.iter_mut().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.weak(label(i, item));
                                        item_controls(
                                            ui,
                                            i,
                                            len,
                                            can_duplicate,
                                            can_remove,
                                            &mut op,
                                        );
                                    });

                                    changed |= body(ui, i, item);
                                });
                            });

                            ui.add_space(24.0);
                        }
                    });
                }

                ArrayLayout::Inline => {
                    ui.horizontal_wrapped(|ui| {
                        if let Some(title) = &title {
                            ui.weak(title);
                        }

                        for (i, item) in items.iter_mut().enumerate() {
                            ui.push_id(i, |ui| {
                                changed |= body(ui, i, item);

                                if can_remove
                                    && ui.small_button("×").on_hover_text("Remove").clicked()
                                {
                                    op = Some(ArrayOp::Removed(i));
                                }
                            });
                        }

                        if addable && ui.add_enabled(can_add, egui::Button::new("➕")).clicked() {
                            op = Some(ArrayOp::Added(len));
                        }
                    });
                }
            }
        });

        if let Some(op) = op {
            op.apply(items);
            changed = true;
        }

        ArrayResponse { changed, op }
    }
}

fn item_controls(
    ui: &mut egui::Ui,
    i: usize,
    len: usize,
    can_duplicate: bool,
    can_remove: bool,
    op: &mut Option<ArrayOp>,
) {
    if ui
        .add_enabled(i > 0, egui::Button::new("⬆").small())
        .on_hover_text("Move up")
        .clicked()
    {
        *op = Some(ArrayOp::Moved { from: i, to: i - 1 });
    }

    if ui
        .add_enabled(i + 1 < len, egui::Button::new("⬇").small())
        .on_hover_text("Move down")
        .clicked()
    {
        *op = Some(ArrayOp::Moved { from: i, to: i + 1 });
    }

    if can_duplicate
        && ui
            .add(egui::Button::new("📋").small())
            .on_hover_text("Duplicate")
            .clicked()
    {
        *op = Some(ArrayOp::Duplicated(i));
    }

    if ui
        .add_enabled(can_remove, egui::Button::new("🗑").small())
        .on_hover_text("Remove")
        .clicked()
    {
        *op = Some(ArrayOp::Removed(i));
    }
}

pub fn optional<T: Default>(
    ui: &mut egui::Ui,
    title: &str,
    value: &mut Option<T>,
    body: impl FnOnce(&mut egui::Ui, &mut T) -> bool,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.heading(title);

        if value.is_none() {
            if ui.button("➕ Add").clicked() {
                *value = Some(T::default());
                changed = true;
            }
        } else if ui
            .add(egui::Button::new("🗑").small())
            .on_hover_text("Remove")
            .clicked()
        {
            *value = None;
            changed = true;
        }
    });

    if let Some(value) = value {
        ui.push_id(title, |ui| changed |= body(ui, value));
    }

    changed
}
