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
    action: &'a mut Option<UiAction>,
    has_fonts: bool,
}

#[derive(Default)]
pub struct TreeView {
    pub hidden_panes: HashSet<PaneId>,
    pub selected_pane: Option<PaneId>,
    tree_state: TreeViewState<PaneId>,
}

impl TreeView {
    pub fn select(&mut self, pane_idx: Option<PaneId>) {
        if let Some(pane_idx) = pane_idx {
            self.tree_state.set_one_selected(pane_idx);
        } else {
            self.deselect_from_view();
        }

        self.selected_pane = pane_idx;
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
                        action: &mut out_action,
                        has_fonts,
                    };

                    let (_, actions) =
                        egui_ltreeview::TreeView::new(ui.make_persistent_id("pane_tree_inspector"))
                            .with_settings(TreeViewSettings {
                                override_indent: Some(12.0),
                                override_striped: Some(true),
                                allow_multi_select: false,
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
                                self.selected_pane = items.first().cloned()
                            }

                            Action::Drag(drag) => {
                                if Self::resolve_move(&layout.tree, drag).is_none() {
                                    drag.remove_drop_marker(ui);
                                }
                            }

                            Action::Move(drag) => {
                                if let Some((source_idx, new_parent, anchor)) =
                                    Self::resolve_move(&layout.tree, drag)
                                {
                                    out_action = Some(UiAction::MovePane {
                                        source_idx,
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
    ) -> Option<(PaneId, Option<PaneId>, Anchor)> {
        let [source_idx] = drag.source[..] else {
            return None;
        };

        let (new_parent, anchor) = pane_tree.resolve_drop_position(drag.target, &drag.position)?;

        pane_tree
            .can_move(source_idx, new_parent, anchor)
            .then_some((source_idx, new_parent, anchor))
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
                    .default_open(!(is_parts_content || is_parts_section))
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
        let hidden_panes = &mut *node_ui.hidden_panes;
        ui.set_min_width(64.0);
        let is_hidden = hidden_panes.contains(&node.pane_idx);

        if !is_hidden && ui.button("Hide").clicked() {
            hidden_panes.insert(node.pane_idx);
            ui.close();
        }

        let has_children = node.all_children().next().is_some();
        if has_children && !is_hidden && ui.button("Hide All").clicked() {
            Self::hide_pane_recursive(node, hidden_panes);
            ui.close();
        }

        if is_hidden && ui.button("Show").clicked() {
            hidden_panes.remove(&node.pane_idx);
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

        let pane = node.pane_idx;
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
            *node_ui.action = Some(UiAction::DuplicatePane(pane));
            ui.close();
        }

        let delete = egui::RichText::new("Delete").color(egui::Color32::from_rgb(224, 96, 96));
        if ui.button(delete).clicked() {
            *node_ui.action = Some(UiAction::DeletePane(pane));
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
