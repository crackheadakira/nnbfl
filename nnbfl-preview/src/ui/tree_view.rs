use crate::pane_tree::PaneId;
use std::collections::HashSet;

use egui_ltreeview::{Action, NodeBuilder, TreeViewBuilder, TreeViewSettings, TreeViewState};
use nnbfl::bflyt::file::BflytSection;

use crate::{
    LayoutData,
    pane_factory::NewPaneKind,
    pane_tree::{Anchor, PaneNode, PaneTree},
    ui::general::UiAction,
};

struct NodeUi<'a> {
    hidden_panes: &'a mut HashSet<PaneId>,
    selection: &'a [PaneId],
    action: &'a mut Option<UiAction>,
    has_fonts: bool,
}

#[derive(Default)]
pub struct TreeView {
    pub hidden_panes: HashSet<PaneId>,

    /// The primary selection
    pub selected_pane: Option<PaneId>,

    /// Every selected pane, primary is included
    pub selection: Vec<PaneId>,

    tree_state: TreeViewState<PaneId>,
}

impl TreeView {
    pub fn select(&mut self, pane_idx: Option<PaneId>) {
        self.select_many(pane_idx.into_iter().collect());
    }

    pub fn select_many(&mut self, panes: Vec<PaneId>) {
        self.selected_pane = panes.first().copied();
        self.tree_state.set_selected(panes.clone());
        self.selection = panes;
    }

    pub fn toggle(&mut self, pane: PaneId) {
        let mut panes = std::mem::take(&mut self.selection);

        let primary = match panes.iter().position(|&p| p == pane) {
            Some(i) => {
                panes.remove(i);
                self.selected_pane
                    .filter(|&p| p != pane)
                    .or(panes.last().copied())
            }
            None => {
                panes.push(pane);
                Some(pane)
            }
        };

        self.select_many(panes);
        self.selected_pane = primary;
    }

    pub fn retain(&mut self, keep: impl Fn(&PaneId) -> bool) {
        let primary = self.selected_pane.filter(&keep);
        let panes: Vec<PaneId> = self.selection.iter().copied().filter(keep).collect();
        let primary = primary.or(panes.first().copied());

        self.select_many(panes);
        self.selected_pane = primary;
    }

    pub fn reveal(&mut self, tree: &PaneTree, panes: &[PaneId]) {
        for &pane in panes {
            let mut parent = tree.parent_map.get(&pane).copied().flatten();

            while let Some(id) = parent {
                self.tree_state.set_openness(id, true);
                parent = tree.parent_map.get(&id).copied().flatten();
            }
        }
    }

    pub fn collapse_empty(&mut self, tree: &PaneTree) {
        for node in tree.iter() {
            if node.all_children().next().is_none()
                && self.tree_state.is_open(&node.pane_idx) == Some(true)
            {
                self.tree_state.set_openness(node.pane_idx, false);
            }
        }
    }

    pub fn targets(&self, pane: PaneId) -> Vec<PaneId> {
        if self.selection.contains(&pane) {
            self.selection.clone()
        } else {
            vec![pane]
        }
    }

    pub fn deselect_from_view(&mut self) {
        self.tree_state.set_selected(Vec::new())
    }

    pub fn show(&mut self, ui: &mut egui::Ui, layout: Option<&mut LayoutData>) -> Option<UiAction> {
        let mut out_action = None;

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if let Some(layout) = layout {
                    let has_fonts = layout
                        .tree
                        .font_list
                        .as_ref()
                        .is_some_and(|f| !f.fonts.is_empty());

                    let add_parent = self
                        .selected_pane
                        .filter(|&id| layout.tree.can_add(Some(id), NewPaneKind::Null))
                        .or_else(|| layout.tree.roots.first().map(|r| r.pane_idx));

                    ui.add_enabled_ui(add_parent.is_some(), |ui| {
                        ui.menu_button("Add", |ui| {
                            if let Some(kind) = Self::kind_menu(ui, has_fonts) {
                                out_action = Some(UiAction::AddPane {
                                    parent: add_parent,
                                    anchor: Anchor::Last,
                                    kind,
                                });
                            }
                        });
                    });

                    let mut node_ui = NodeUi {
                        hidden_panes: &mut self.hidden_panes,
                        selection: &self.selection,
                        action: &mut out_action,
                        has_fonts,
                    };

                    let (_, actions) =
                        egui_ltreeview::TreeView::new(ui.make_persistent_id("pane_tree_inspector"))
                            .with_settings(TreeViewSettings {
                                override_indent: Some(12.0),
                                override_striped: Some(true),
                                allow_multi_select: true,
                                allow_drag_and_drop: true,
                                ..Default::default()
                            })
                            .show_state(ui, &mut self.tree_state, |builder| {
                                for root in layout.tree.roots.iter() {
                                    Self::draw_pane_node(root, None, builder, &mut node_ui);
                                }
                            });

                    for action in actions.iter() {
                        match action {
                            Action::SetSelected(items) => {
                                let added = items.iter().find(|i| !self.selection.contains(i));
                                let kept = self.selected_pane.filter(|p| items.contains(p));

                                self.selected_pane =
                                    added.copied().or(kept).or(items.first().copied());
                                self.selection = items.clone();
                            }

                            Action::Drag(drag) => {
                                if Self::resolve_move(&layout.tree, drag).is_none() {
                                    drag.remove_drop_marker(ui);
                                }
                            }

                            Action::Move(drag) => {
                                if let Some((sources, new_parent, anchor)) =
                                    Self::resolve_move(&layout.tree, drag)
                                {
                                    out_action = Some(UiAction::MovePanes {
                                        sources,
                                        new_parent,
                                        anchor,
                                    });
                                }
                            }

                            _ => {}
                        }
                    }
                } else {
                    ui.label("No .bflyt loaded...");
                }
            });

        out_action
    }

    fn resolve_move(
        pane_tree: &PaneTree,
        drag: &egui_ltreeview::DragAndDrop<PaneId>,
    ) -> Option<(Vec<PaneId>, Option<PaneId>, Anchor)> {
        let sources = pane_tree.selection_roots(&drag.source);
        let (new_parent, anchor) = pane_tree.resolve_drop_position(drag.target, &drag.position)?;

        let anchored_to_source = matches!(anchor, Anchor::Before(id) | Anchor::After(id) if sources.len() > 1 && sources.contains(&id));

        (!sources.is_empty()
            && !anchored_to_source
            && sources
                .iter()
                .all(|&source| pane_tree.can_move(source, new_parent, anchor)))
        .then_some((sources, new_parent, anchor))
    }

    fn draw_pane_node(
        node: &PaneNode,
        parent: Option<PaneId>,
        builder: &mut TreeViewBuilder<PaneId>,
        node_ui: &mut NodeUi,
    ) {
        let i = node.pane_idx;

        let is_parts_section = matches!(node.section, BflytSection::PartsPane(_));

        let is_parts_content = node.parts_source.is_some();
        let is_hidden = node_ui.hidden_panes.contains(&i);

        let label_text = if is_parts_content {
            format!("[{}] {} (linked)", node.kind, node.label)
        } else {
            format!("[{}] {}", node.kind, node.label)
        };

        let label = if is_parts_content {
            egui::RichText::new(label_text).weak()
        } else {
            egui::RichText::new(label_text)
        };

        if !is_parts_content || node.all_children().next().is_some() {
            builder.node(
                NodeBuilder::dir(i)
                    .default_open(
                        !(is_parts_content || is_parts_section)
                            && node.all_children().next().is_some(),
                    )
                    .drop_allowed(!is_parts_content)
                    .activatable(!is_parts_content)
                    .height(24.0)
                    .context_menu(|ui| {
                        Self::draw_pane_context_menu(ui, node, parent, node_ui);
                    })
                    .label_ui(|ui| {
                        ui.add(egui::Label::new(label.clone()).selectable(false));

                        if is_hidden {
                            ui.add(egui::Label::new("Hidden").selectable(false));
                        }
                    }),
            );

            for child in node.all_children() {
                Self::draw_pane_node(child, Some(i), builder, node_ui);
            }

            builder.close_dir();
        } else {
            builder.node(
                NodeBuilder::leaf(i)
                    .activatable(!is_parts_content)
                    .drop_allowed(!is_parts_content)
                    .height(24.0)
                    .context_menu(|ui| {
                        Self::draw_pane_context_menu(ui, node, parent, node_ui);
                    })
                    .label_ui(|ui| {
                        ui.add(egui::Label::new(label.clone()).selectable(false));

                        if is_hidden {
                            ui.add(egui::Label::new("Hidden").selectable(false));
                        }
                    }),
            );
        }
    }

    fn draw_pane_context_menu(
        ui: &mut egui::Ui,
        node: &PaneNode,
        parent: Option<PaneId>,
        node_ui: &mut NodeUi,
    ) {
        let pane = node.pane_idx;

        let targets = if node_ui.selection.contains(&pane) {
            node_ui.selection.to_vec()
        } else {
            vec![pane]
        };

        let hidden_panes = &mut *node_ui.hidden_panes;
        ui.set_min_width(64.0);
        let is_hidden = hidden_panes.contains(&pane);

        if !is_hidden && ui.button("Hide").clicked() {
            hidden_panes.extend(&targets);
            ui.close();
        }

        let has_children = node.all_children().next().is_some();
        if has_children && !is_hidden && ui.button("Hide All").clicked() {
            Self::hide_pane_recursive(node, hidden_panes);
            ui.close();
        }

        if is_hidden && ui.button("Show").clicked() {
            for target in &targets {
                hidden_panes.remove(target);
            }

            ui.close();
        }

        if has_children && is_hidden && ui.button("Show All").clicked() {
            Self::show_pane_recursive(node, hidden_panes);
            ui.close();
        }

        if node.parts_source.is_some() {
            return;
        }

        ui.separator();

        let has_fonts = node_ui.has_fonts;
        let mut add = |parent, anchor, kind| {
            *node_ui.action = Some(UiAction::AddPane {
                parent,
                anchor,
                kind,
            });
        };

        ui.menu_button("Add child", |ui| {
            if let Some(kind) = Self::kind_menu(ui, has_fonts) {
                add(Some(pane), Anchor::Last, kind);
            }
        });

        ui.add_enabled_ui(parent.is_some(), |ui| {
            ui.menu_button("Add sibling", |ui| {
                if let Some(kind) = Self::kind_menu(ui, has_fonts) {
                    add(parent, Anchor::After(pane), kind);
                }
            });
        });

        if ui.button("Duplicate").clicked() {
            *node_ui.action = Some(UiAction::DuplicatePanes(targets.clone()));

            ui.close();
        }

        if ui.button("Duplicate…").clicked() {
            *node_ui.action = Some(UiAction::OpenDuplicateDialog(pane));
            ui.close();
        }

        let delete = egui::RichText::new("Delete").color(egui::Color32::from_rgb(224, 96, 96));

        if ui.button(delete).clicked() {
            *node_ui.action = Some(UiAction::DeletePanes(targets));

            ui.close();
        }
    }

    fn kind_menu(ui: &mut egui::Ui, has_fonts: bool) -> Option<NewPaneKind> {
        let mut picked = None;

        for kind in NewPaneKind::ALL {
            let button = ui
                .add_enabled(
                    has_fonts || !kind.needs_font(),
                    egui::Button::new(kind.label()),
                )
                .on_disabled_hover_text("The layout has no fonts");

            if button.clicked() {
                picked = Some(kind);
                ui.close();
            }
        }

        picked
    }

    fn show_pane_recursive(node: &PaneNode, hidden_panes: &mut HashSet<PaneId>) {
        hidden_panes.remove(&node.pane_idx);

        for child in node.descendants() {
            hidden_panes.remove(&child);
        }
    }

    fn hide_pane_recursive(node: &PaneNode, hidden_panes: &mut HashSet<PaneId>) {
        hidden_panes.insert(node.pane_idx);

        for child in node.descendants() {
            hidden_panes.insert(child);
        }
    }
}
