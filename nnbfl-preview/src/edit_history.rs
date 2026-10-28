use std::collections::HashSet;

use crate::{
    pane_factory::{self, NewPaneKind},
    pane_tree::{Anchor, PaneId},
};
use nnbfl::{
    bflyt::{
        file::{BflytSection, ControlSourceElement, GroupElement},
        list::MaterialList,
    },
    ui2d::{
        types::{Vector2f, Vector3f},
        userdata::UserDataArray,
    },
};

use crate::{
    pane_tree::{DirtyFlags, PaneNode, PaneTree},
    traits::Displaying,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaneTransform {
    pub translation: Vector3f,
    pub size: Vector2f,
    pub rotation_z: f32,
}

pub enum PaneEdit {
    Delete {
        target_idx: PaneId,
    },
    Duplicate {
        source_idx: PaneId,
    },
    Add {
        parent: Option<PaneId>,
        anchor: Anchor,
        kind: NewPaneKind,
    },
    Move {
        source_idx: PaneId,
        new_parent: Option<PaneId>,
        anchor: Anchor,
    },
    Rename {
        target_idx: PaneId,
        name: String,
    },
}

impl PaneEdit {
    pub fn apply(self, tree: &mut PaneTree) -> Option<AppliedCommand> {
        match self {
            PaneEdit::Delete { target_idx } => {
                let (parent_idx, position) = tree.sibling_position(target_idx)?;
                let meta_before = LayoutMeta::of(tree);
                let node = tree.remove_node(target_idx)?;

                let names: HashSet<String> = node
                    .iter()
                    .filter(|n| n.parts_source.is_none())
                    .map(|n| n.label.trim_end_matches('\0').to_string())
                    .collect();

                tree.drop_references(&names);

                Some(AppliedCommand::Batch(vec![
                    AppliedCommand::Removed(Box::new(PendingRemoval {
                        parent_idx,
                        position,
                        node,
                    })),
                    AppliedCommand::ReplaceLayoutMeta {
                        before: Box::new(meta_before),
                        after: Box::new(LayoutMeta::of(tree)),
                    },
                ]))
            }

            PaneEdit::Duplicate { source_idx } => {
                let new_idx = tree.duplicate_node(source_idx)?;
                Some(AppliedCommand::Inserted(new_idx))
            }

            PaneEdit::Add {
                parent,
                anchor,
                kind,
            } => {
                if !tree.can_add(parent, kind) {
                    return None;
                }

                let meta_before = LayoutMeta::of(tree);
                let name = tree.unique_name(kind.name_prefix(), &HashSet::new());
                let section = pane_factory::new_pane_section(
                    kind,
                    &name,
                    tree.material_list.get_or_insert_default(),
                );

                let node = tree.new_node(section, parent);
                let position = tree.anchor_position(parent, anchor)?;
                let idx = tree.insert_node_at(parent, position, node);
                tree.mark_pane_dirty(idx);

                Some(AppliedCommand::Batch(vec![
                    AppliedCommand::ReplaceLayoutMeta {
                        before: Box::new(meta_before),
                        after: Box::new(LayoutMeta::of(tree)),
                    },
                    AppliedCommand::Inserted(idx),
                ]))
            }

            PaneEdit::Move {
                source_idx,
                new_parent,
                anchor,
            } => {
                let (old_parent, old_anchor) = tree.move_node(source_idx, new_parent, anchor)?;

                Some(AppliedCommand::Moved {
                    pane_idx: source_idx,
                    old_parent,
                    old_anchor,
                })
            }

            PaneEdit::Rename { target_idx, name } => {
                if tree.pane_name_error(&name, Some(target_idx)).is_some() {
                    return None;
                }

                let node = tree.find_by_idx(target_idx)?;
                if node.parts_source.is_some() {
                    return None;
                }

                let pane_before = PaneSnapshot::of(node);
                let old = pane_before.label.trim_end_matches('\0').to_string();
                let meta_before = LayoutMeta::of(tree);

                tree.find_by_idx_mut(target_idx)?.label = name.clone();
                tree.rename_references(&old, &name);
                tree.rebuild_maps();

                Some(AppliedCommand::Batch(vec![
                    AppliedCommand::ReplacePane {
                        pane_idx: target_idx,
                        before: Box::new(pane_before),
                        after: Box::new(PaneSnapshot::of(tree.find_by_idx(target_idx)?)),
                    },
                    AppliedCommand::ReplaceLayoutMeta {
                        before: Box::new(meta_before),
                        after: Box::new(LayoutMeta::of(tree)),
                    },
                ]))
            }
        }
    }
}

#[derive(Clone)]
pub struct PaneSnapshot {
    pub section: BflytSection,
    pub label: String,
    pub user_data: Option<UserDataArray>,
}

impl PaneSnapshot {
    pub fn of(node: &PaneNode) -> Self {
        Self {
            section: node.section.clone(),
            label: node.label.clone(),
            user_data: node.user_data.clone(),
        }
    }
}

#[derive(Clone)]
pub struct LayoutMeta {
    pub user_data: Option<UserDataArray>,
    pub group: GroupElement,
    pub control_source: Option<ControlSourceElement>,
    pub material_list: Option<MaterialList>,
}

impl LayoutMeta {
    pub fn of(tree: &PaneTree) -> Self {
        Self {
            user_data: tree.user_data.clone(),
            group: tree.group.clone(),
            control_source: tree.control_source.clone(),
            material_list: tree.material_list.clone(),
        }
    }
}

fn changed_materials(a: &Option<MaterialList>, b: &Option<MaterialList>) -> Vec<u16> {
    let a = a.as_ref().map_or(&[][..], |l| &l.materials);
    let b = b.as_ref().map_or(&[][..], |l| &l.materials);

    if a.len() != b.len() {
        return (0..a.len().max(b.len()) as u16).collect();
    }

    (0..a.len())
        .filter(|&i| format!("{:?}", a[i]) != format!("{:?}", b[i]))
        .map(|i| i as u16)
        .collect()
}

pub struct PendingRemoval {
    parent_idx: Option<PaneId>,
    position: usize,
    node: PaneNode,
}

pub enum AppliedCommand {
    Inserted(PaneId),
    Removed(Box<PendingRemoval>),
    Transformed {
        pane_idx: PaneId,
        before: PaneTransform,
        after: PaneTransform,
    },
    Moved {
        pane_idx: PaneId,
        old_parent: Option<PaneId>,
        old_anchor: Anchor,
    },
    ReplacePane {
        pane_idx: PaneId,
        before: Box<PaneSnapshot>,
        after: Box<PaneSnapshot>,
    },
    ReplaceLayoutMeta {
        before: Box<LayoutMeta>,
        after: Box<LayoutMeta>,
    },
    Batch(Vec<AppliedCommand>),
}

impl AppliedCommand {
    pub fn invert(self, tree: &mut PaneTree) -> Option<AppliedCommand> {
        match self {
            AppliedCommand::Inserted(pane_idx) => {
                let (parent_idx, position) = tree.sibling_position(pane_idx)?;
                let node = tree.remove_node(pane_idx)?;
                Some(AppliedCommand::Removed(Box::new(PendingRemoval {
                    parent_idx,
                    position,
                    node,
                })))
            }

            AppliedCommand::Removed(removal) => {
                let pane_idx =
                    tree.insert_node_at(removal.parent_idx, removal.position, removal.node);
                Some(AppliedCommand::Inserted(pane_idx))
            }
            AppliedCommand::Transformed {
                pane_idx,
                before,
                after,
            } => {
                let node = tree.find_by_idx_mut(pane_idx)?;
                let base = node.section.get_base_pane_mut()?;

                base.translation = before.translation;
                base.size = before.size;
                base.rotation.z = before.rotation_z;

                node.mark_transform_dirty();
                tree.recompute_dirty();

                Some(AppliedCommand::Transformed {
                    pane_idx,
                    before: after,
                    after: before,
                })
            }
            AppliedCommand::Moved {
                pane_idx,
                old_parent,
                old_anchor,
            } => {
                let (prev_parent, prev_anchor) =
                    tree.move_node(pane_idx, old_parent, old_anchor)?;

                Some(AppliedCommand::Moved {
                    pane_idx,
                    old_parent: prev_parent,
                    old_anchor: prev_anchor,
                })
            }

            AppliedCommand::ReplacePane {
                pane_idx,
                before,
                after,
            } => {
                let node = tree.find_by_idx_mut(pane_idx)?;
                let renamed = node.label != before.label;

                node.section = before.section.clone();
                node.label = before.label.clone();
                node.user_data = before.user_data.clone();
                node.dirty = DirtyFlags::all();
                tree.mark_pane_dirty(pane_idx);

                if renamed {
                    tree.rebuild_maps();
                }

                Some(AppliedCommand::ReplacePane {
                    pane_idx,
                    before: after,
                    after: before,
                })
            }

            AppliedCommand::ReplaceLayoutMeta { before, after } => {
                let changed = changed_materials(&tree.material_list, &before.material_list);

                tree.user_data = before.user_data.clone();
                tree.group = before.group.clone();
                tree.control_source = before.control_source.clone();
                tree.material_list = before.material_list.clone();

                for material in changed {
                    tree.mark_material_dirty(material);
                }

                Some(AppliedCommand::ReplaceLayoutMeta {
                    before: after,
                    after: before,
                })
            }

            AppliedCommand::Batch(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());

                for command in commands.into_iter().rev() {
                    inverses.push(command.invert(tree)?);
                }

                Some(AppliedCommand::Batch(inverses))
            }
        }
    }

    pub fn resulting_panes(&self) -> Vec<PaneId> {
        match self {
            AppliedCommand::Inserted(idx) => vec![*idx],
            AppliedCommand::Removed(_) => Vec::new(),
            AppliedCommand::Transformed { pane_idx, .. } => vec![*pane_idx],
            AppliedCommand::Moved { pane_idx, .. } => vec![*pane_idx],
            AppliedCommand::ReplacePane { pane_idx, .. } => vec![*pane_idx],
            AppliedCommand::ReplaceLayoutMeta { .. } => Vec::new(),
            AppliedCommand::Batch(commands) => {
                let mut panes = Vec::new();

                for pane in commands.iter().flat_map(AppliedCommand::resulting_panes) {
                    if !panes.contains(&pane) {
                        panes.push(pane);
                    }
                }

                panes
            }
        }
    }
}

pub struct EditHistory {
    undo_stack: Vec<AppliedCommand>,
    redo_stack: Vec<AppliedCommand>,
    limit: usize,
}

impl EditHistory {
    pub fn new(limit: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit,
        }
    }

    pub fn perform(&mut self, tree: &mut PaneTree, edit: PaneEdit) -> Vec<PaneId> {
        self.perform_all(tree, vec![edit])
    }

    pub fn perform_all(&mut self, tree: &mut PaneTree, edits: Vec<PaneEdit>) -> Vec<PaneId> {
        let mut applied: Vec<AppliedCommand> = edits
            .into_iter()
            .filter_map(|edit| edit.apply(tree))
            .collect();

        let command = match applied.len() {
            0 => return Vec::new(),
            1 => applied.remove(0),
            _ => AppliedCommand::Batch(applied),
        };

        let panes = command.resulting_panes();
        self.record(command);

        panes
    }

    pub fn undo(&mut self, tree: &mut PaneTree) -> Vec<PaneId> {
        let Some(applied) = self.undo_stack.pop() else {
            return Vec::new();
        };

        let Some(inverse) = applied.invert(tree) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        self.redo_stack.push(inverse);

        panes
    }

    pub fn redo(&mut self, tree: &mut PaneTree) -> Vec<PaneId> {
        let Some(applied) = self.redo_stack.pop() else {
            return Vec::new();
        };

        let Some(inverse) = applied.invert(tree) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        self.push_undo(inverse);

        panes
    }

    fn push_undo(&mut self, applied: AppliedCommand) {
        if self.undo_stack.len() >= self.limit {
            self.undo_stack.remove(0);
        }

        self.undo_stack.push(applied);
    }

    pub fn record(&mut self, applied: AppliedCommand) {
        self.redo_stack.clear();
        self.push_undo(applied);
    }

    pub fn record_transform(
        &mut self,
        pane_idx: PaneId,
        before: PaneTransform,
        after: PaneTransform,
    ) {
        if before == after {
            return;
        }

        self.record(AppliedCommand::Transformed {
            pane_idx,
            before,
            after,
        });
    }
}
