use std::collections::HashSet;

use crate::{
    ops::duplicate::{self, DuplicateNames, DuplicateOptions},
    pane_factory::{self, NewPaneKind},
    pane_tree::{Anchor, PaneId},
    ui::timeline::TimelineState,
};
use nnbfl::{
    bflan::anim_info::{AnimContent, AnimInfo, AnimType, PaneAnimInfo},
    bflyt::{
        file::{BflytSection, ControlSourceElement, GroupElement},
        list::{MATERIAL_NAME_LEN, Material, MaterialList},
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

pub struct EditCtx<'a> {
    pub tree: &'a mut PaneTree,
    pub timeline: &'a mut TimelineState,
}

pub enum PaneEdit {
    Delete {
        target_idx: PaneId,
    },
    Duplicate {
        source_idx: PaneId,
        names: Option<DuplicateNames>,
        options: DuplicateOptions,
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
    pub fn apply(self, ctx: &mut EditCtx) -> Option<AppliedCommand> {
        let tree = &mut *ctx.tree;

        match self {
            PaneEdit::Delete { target_idx } => {
                let (parent_idx, position) = tree.sibling_position(target_idx)?;
                let node = tree.remove_node(target_idx)?;

                let names: HashSet<String> = node
                    .iter()
                    .filter(|n| n.parts_source.is_none())
                    .map(|n| n.label.trim_end_matches('\0').to_string())
                    .collect();

                let mut commands = vec![AppliedCommand::Removed(Box::new(PendingRemoval {
                    parent_idx,
                    position,
                    node,
                }))];

                let AppliedCommand::Removed(removal) = &commands[0] else {
                    unreachable!()
                };

                let mut orphans: Vec<u16> = removal
                    .node
                    .iter()
                    .filter(|n| n.parts_source.is_none() || n.is_parts_overridden)
                    .flat_map(PaneNode::material_indices)
                    .filter(|index| !tree.material_users.contains_key(index))
                    .collect();

                orphans.sort_unstable();
                orphans.dedup();

                let mut material_names = HashSet::new();

                for &index in orphans.iter().rev() {
                    if let Some(material) = tree.remove_material(index) {
                        material_names
                            .insert(material.material_name.trim_end_matches('\0').to_string());
                        commands.push(AppliedCommand::RemovedMaterial {
                            index,
                            material: Box::new(material),
                        });
                    }
                }

                commands.extend(edit_anims(ctx.timeline, ctx.tree, |info| {
                    let before = info.contents.len();

                    info.contents.retain(|content| {
                        let name = content.name.trim_end_matches('\0');

                        !(is_pane_content(content) && names.contains(name)
                            || content.anim_type == AnimType::Material
                                && material_names.contains(name))
                    });

                    info.contents.len() != before
                }));

                let tree = &mut *ctx.tree;
                let meta_before = LayoutMeta::of(tree);
                tree.drop_references(&names);

                commands.push(AppliedCommand::ReplaceLayoutMeta {
                    before: Box::new(meta_before),
                    after: Box::new(LayoutMeta::of(tree)),
                });

                Some(AppliedCommand::Batch(commands))
            }

            PaneEdit::Duplicate {
                source_idx,
                names,
                options,
            } => {
                let names = names.or_else(|| duplicate::plan(tree, source_idx, None))?;
                duplicate::apply(ctx, source_idx, names, options)
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
                let material_renames = material_renames(tree, target_idx, &old, &name);

                tree.find_by_idx_mut(target_idx)?.label = name.clone();
                tree.rename_references(&old, &name);

                if let Some(list) = &mut tree.material_list {
                    for (index, _, new) in &material_renames {
                        list.materials[*index as usize].material_name = new.clone();
                    }
                }

                for (index, _, _) in &material_renames {
                    tree.mark_material_dirty(*index);
                }

                tree.rebuild_maps();

                let pane_after = PaneSnapshot::of(tree.find_by_idx(target_idx)?);
                let meta_after = LayoutMeta::of(tree);

                let mut commands = vec![
                    AppliedCommand::ReplacePane {
                        pane_idx: target_idx,
                        before: Box::new(pane_before),
                        after: Box::new(pane_after),
                    },
                    AppliedCommand::ReplaceLayoutMeta {
                        before: Box::new(meta_before),
                        after: Box::new(meta_after),
                    },
                ];

                commands.extend(edit_anims(ctx.timeline, ctx.tree, |info| {
                    let mut changed = false;

                    for content in &mut info.contents {
                        let current = content.name.trim_end_matches('\0');

                        let new = if is_pane_content(content) && current == old {
                            Some(name.clone())
                        } else if content.anim_type == AnimType::Material {
                            material_renames
                                .iter()
                                .find(|(_, from, _)| from == current)
                                .map(|(_, _, to)| to.clone())
                        } else {
                            None
                        };

                        if let Some(new) = new {
                            content.name = new;
                            changed = true;
                        }
                    }

                    changed
                }));

                Some(AppliedCommand::Batch(commands))
            }
        }
    }
}

fn is_pane_content(content: &AnimContent) -> bool {
    matches!(content.anim_type, AnimType::Pane | AnimType::PaneExt)
}

fn material_renames(
    tree: &PaneTree,
    pane: PaneId,
    old: &str,
    new: &str,
) -> Vec<(u16, String, String)> {
    let Some(node) = tree.find_by_idx(pane) else {
        return Vec::new();
    };

    let Some(list) = &tree.material_list else {
        return Vec::new();
    };

    let taken = |name: &str| {
        list.materials
            .iter()
            .any(|m| m.material_name.trim_end_matches('\0') == name)
    };

    let mut renames: Vec<(u16, String, String)> = Vec::new();

    for index in node.material_indices() {
        let Some(material) = list.materials.get(index as usize) else {
            continue;
        };

        let current = material.material_name.trim_end_matches('\0');
        let own = tree
            .material_users
            .get(&index)
            .is_some_and(|users| users == &[pane]);

        let Some(rest) = current.strip_prefix(old) else {
            continue;
        };

        if !own {
            continue;
        }

        let renamed = format!("{new}{rest}");

        if renamed.len() <= MATERIAL_NAME_LEN
            && !taken(&renamed)
            && !renames.iter().any(|(i, _, n)| *i == index || *n == renamed)
        {
            renames.push((index, current.to_string(), renamed));
        }
    }

    renames
}

pub(crate) fn edit_anims(
    timeline: &mut TimelineState,
    tree: &mut PaneTree,
    mut edit: impl FnMut(&mut PaneAnimInfo) -> bool,
) -> Vec<AppliedCommand> {
    let mut commands = Vec::new();
    let mut reshaped = false;

    for (anim, instance) in timeline.anim_player.anims.iter_mut().enumerate() {
        let before = instance.bflan.anim_info.clone();

        if edit(&mut instance.bflan.anim_info) {
            reshaped |= !same_tracks(&before, &instance.bflan.anim_info);

            commands.push(AppliedCommand::ReplaceAnimInfo {
                anim,
                before: Box::new(before),
                after: Box::new(instance.bflan.anim_info.clone()),
            });
        }
    }

    if !commands.is_empty() {
        anims_changed(timeline, tree, reshaped);
    }

    commands
}

fn anims_changed(timeline: &mut TimelineState, tree: &mut PaneTree, reshaped: bool) {
    timeline.anim_player.invalidate();
    tree.animation_binding_revision = tree.animation_binding_revision.wrapping_add(1);

    if reshaped {
        timeline.reset_tracks();
    }
}

fn same_tracks(a: &PaneAnimInfo, b: &PaneAnimInfo) -> bool {
    let shape = |info: &AnimInfo| match info {
        AnimInfo::Standard { targets, .. } => targets.len(),
        AnimInfo::ExtendedUserData { data, .. } => data.len(),
    };

    a.contents.len() == b.contents.len()
        && a.contents.iter().zip(&b.contents).all(|(a, b)| {
            a.infos.len() == b.infos.len()
                && a.infos
                    .iter()
                    .zip(&b.infos)
                    .all(|(a, b)| shape(a) == shape(b))
        })
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
    RemovedMaterial {
        index: u16,
        material: Box<Material>,
    },
    InsertedMaterial {
        index: u16,
    },
    ReplaceAnimInfo {
        anim: usize,
        before: Box<PaneAnimInfo>,
        after: Box<PaneAnimInfo>,
    },
    Batch(Vec<AppliedCommand>),
}

impl AppliedCommand {
    pub fn invert(self, ctx: &mut EditCtx) -> Option<AppliedCommand> {
        let tree = &mut *ctx.tree;

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

            AppliedCommand::RemovedMaterial { index, material } => {
                tree.insert_material(index, *material);
                Some(AppliedCommand::InsertedMaterial { index })
            }

            AppliedCommand::InsertedMaterial { index } => {
                let material = tree.remove_material(index)?;
                Some(AppliedCommand::RemovedMaterial {
                    index,
                    material: Box::new(material),
                })
            }

            AppliedCommand::ReplaceAnimInfo {
                anim,
                before,
                after,
            } => {
                let instance = ctx.timeline.anim_player.anims.get_mut(anim)?;
                let reshaped = !same_tracks(&instance.bflan.anim_info, &before);
                instance.bflan.anim_info = (*before).clone();
                anims_changed(ctx.timeline, ctx.tree, reshaped);

                Some(AppliedCommand::ReplaceAnimInfo {
                    anim,
                    before: after,
                    after: before,
                })
            }

            AppliedCommand::Batch(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());

                for command in commands.into_iter().rev() {
                    inverses.push(command.invert(ctx)?);
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
            AppliedCommand::ReplaceLayoutMeta { .. }
            | AppliedCommand::RemovedMaterial { .. }
            | AppliedCommand::InsertedMaterial { .. }
            | AppliedCommand::ReplaceAnimInfo { .. } => Vec::new(),
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

pub struct AnimSession {
    pub anim: usize,
    pub baseline: PaneAnimInfo,
}

struct Step {
    label: String,
    command: AppliedCommand,
}

pub struct EditHistory {
    undo_stack: Vec<Step>,
    redo_stack: Vec<Step>,
    limit: usize,
    dropped: bool,
}

impl EditHistory {
    pub fn new(limit: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            limit,
            dropped: false,
        }
    }

    pub fn perform(&mut self, ctx: &mut EditCtx, edit: PaneEdit) -> Vec<PaneId> {
        self.perform_all(ctx, vec![edit])
    }

    pub fn perform_all(&mut self, ctx: &mut EditCtx, edits: Vec<PaneEdit>) -> Vec<PaneId> {
        let label = describe(&edits, ctx.tree);

        let mut applied: Vec<AppliedCommand> = edits
            .into_iter()
            .filter_map(|edit| edit.apply(ctx))
            .collect();

        let command = match applied.len() {
            0 => return Vec::new(),
            1 => applied.remove(0),
            _ => AppliedCommand::Batch(applied),
        };

        let panes = command.resulting_panes();
        self.record(label, command);

        panes
    }

    pub fn undo(&mut self, ctx: &mut EditCtx) -> Vec<PaneId> {
        let Some(step) = self.undo_stack.pop() else {
            return Vec::new();
        };

        let Some(inverse) = step.command.invert(ctx) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        self.redo_stack.push(Step {
            label: step.label,
            command: inverse,
        });

        panes
    }

    pub fn redo(&mut self, ctx: &mut EditCtx) -> Vec<PaneId> {
        let Some(step) = self.redo_stack.pop() else {
            return Vec::new();
        };

        let Some(inverse) = step.command.invert(ctx) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        self.push_undo(Step {
            label: step.label,
            command: inverse,
        });

        panes
    }

    pub fn position(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn steps(&self) -> impl Iterator<Item = &str> {
        self.undo_stack
            .iter()
            .chain(self.redo_stack.iter().rev())
            .map(|step| step.label.as_str())
    }

    pub fn dropped(&self) -> bool {
        self.dropped
    }

    fn push_undo(&mut self, step: Step) {
        if self.undo_stack.len() >= self.limit {
            self.undo_stack.remove(0);
            self.dropped = true;
        }

        self.undo_stack.push(step);
    }

    pub fn record(&mut self, label: String, command: AppliedCommand) {
        self.redo_stack.clear();
        self.push_undo(Step { label, command });
    }

    pub fn record_transform(
        &mut self,
        label: String,
        pane_idx: PaneId,
        before: PaneTransform,
        after: PaneTransform,
    ) {
        if before == after {
            return;
        }

        self.record(
            label,
            AppliedCommand::Transformed {
                pane_idx,
                before,
                after,
            },
        );
    }

    pub fn record_anim(
        &mut self,
        label: String,
        anim: usize,
        before: PaneAnimInfo,
        after: PaneAnimInfo,
    ) {
        self.record(
            label,
            AppliedCommand::ReplaceAnimInfo {
                anim,
                before: Box::new(before),
                after: Box::new(after),
            },
        );
    }
}

fn describe(edits: &[PaneEdit], tree: &PaneTree) -> String {
    let name = |id: &PaneId| {
        tree.find_by_idx(*id)
            .map(|n| n.label.trim_end_matches('\0').to_string())
            .unwrap_or_default()
    };

    let verb = |edit: &PaneEdit| match edit {
        PaneEdit::Delete { .. } => "Delete",
        PaneEdit::Duplicate { .. } => "Duplicate",
        PaneEdit::Add { .. } => "Add",
        PaneEdit::Move { .. } => "Reorder",
        PaneEdit::Rename { .. } => "Rename",
    };

    match edits {
        [] => String::new(),
        [edit] => match edit {
            PaneEdit::Delete { target_idx: id }
            | PaneEdit::Duplicate { source_idx: id, .. }
            | PaneEdit::Move { source_idx: id, .. } => format!("{} {}", verb(edit), name(id)),
            PaneEdit::Add { kind, .. } => format!("Add {}", kind.label()),
            PaneEdit::Rename {
                target_idx,
                name: new,
            } => {
                format!("Rename {} to {new}", name(target_idx))
            }
        },
        [first, rest @ ..] if rest.iter().all(|e| verb(e) == verb(first)) => {
            format!("{} {} panes", verb(first), edits.len())
        }
        _ => format!("{} edits", edits.len()),
    }
}
