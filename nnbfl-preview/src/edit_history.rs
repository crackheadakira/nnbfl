use std::collections::{HashSet, VecDeque};

use tomolib::formats::bntx::Texture;

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
        list::{MATERIAL_NAME_LEN, Material, MaterialList, TextureList},
    },
    ui2d::{
        types::{Vector2f, Vector3f},
        userdata::UserDataArray,
    },
};

use crate::{
    font::glyph_atlas::GlyphData,
    pane_tree::{DirtyFlags, PaneNode, PaneTree, take_parts_content},
    renderer::textured_quad::TexturedQuad,
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
    pub glyphs: &'a mut GlyphData,
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
    AddMaterial,
    OverrideSection {
        owner: PaneId,
        pane: String,
        toggle: OverrideToggle,
    },
    DuplicateMaterial {
        index: u16,
    },
    DeleteMaterial {
        index: u16,
    },
    RenameMaterial {
        index: u16,
        name: String,
    },
    ImportTexture {
        texture: Box<Texture>,
    },
    EditAnim {
        anim: usize,
        label: String,
        info: Box<PaneAnimInfo>,
    },
    ReplaceTexture {
        index: usize,
        texture: Box<Texture>,
    },
}

impl PaneEdit {
    pub fn touches_panes(&self) -> bool {
        !matches!(
            self,
            PaneEdit::AddMaterial
                | PaneEdit::OverrideSection { .. }
                | PaneEdit::DuplicateMaterial { .. }
                | PaneEdit::DeleteMaterial { .. }
                | PaneEdit::RenameMaterial { .. }
                | PaneEdit::ImportTexture { .. }
                | PaneEdit::ReplaceTexture { .. }
                | PaneEdit::EditAnim { .. }
        )
    }

    pub fn target_pane(&self) -> Option<PaneId> {
        match self {
            PaneEdit::Delete { target_idx } | PaneEdit::Rename { target_idx, .. } => {
                Some(*target_idx)
            }
            PaneEdit::Duplicate { source_idx, .. } | PaneEdit::Move { source_idx, .. } => {
                Some(*source_idx)
            }
            PaneEdit::Add { parent, .. } => *parent,
            PaneEdit::OverrideSection { owner, .. } => Some(*owner),
            PaneEdit::AddMaterial
            | PaneEdit::DuplicateMaterial { .. }
            | PaneEdit::DeleteMaterial { .. }
            | PaneEdit::RenameMaterial { .. }
            | PaneEdit::ImportTexture { .. }
            | PaneEdit::ReplaceTexture { .. }
            | PaneEdit::EditAnim { .. } => None,
        }
    }
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

                let mut orphans: Vec<u16> = node
                    .iter()
                    .filter(|n| n.parts_source.is_none())
                    .flat_map(PaneNode::material_indices)
                    .filter(|index| !tree.material_users.contains_key(index))
                    .collect();

                let mut commands = vec![AppliedCommand::Removed(Box::new(PendingRemoval::new(
                    parent_idx, position, node,
                )))];

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

            PaneEdit::EditAnim { anim, info, .. } => {
                let instance = ctx.timeline.anim_player.anims.get_mut(anim)?;
                let before = std::mem::replace(&mut instance.bflan.anim_info, (*info).clone());
                let reshaped = !same_tracks(&before, &info);
                let shrank = info.contents.len() < before.contents.len();

                ctx.timeline.anim_player.invalidate();
                tree.animation_binding_revision = tree.animation_binding_revision.wrapping_add(1);

                if reshaped {
                    ctx.timeline.selected_key = None;
                    ctx.timeline.pending_key_edit = None;
                    ctx.timeline.pending_slope_edit = None;
                    ctx.timeline.hidden_tracks.clear();
                }

                if shrank {
                    ctx.timeline.expanded_anim_panes.clear();
                }

                Some(AppliedCommand::ReplaceAnimInfo {
                    anim,
                    before: Box::new(before),
                    after: info,
                })
            }

            PaneEdit::ImportTexture { texture } => {
                let textures = &mut tree.main_bntx.as_mut()?.textures;
                let index = textures.len();
                tree.texture_changes.push(texture.name.clone());
                textures.push((*texture).clone());
                tree.bntx_edited = true;

                Some(AppliedCommand::ReplaceBntxTexture {
                    index,
                    before: None,
                    after: Some(texture),
                })
            }

            PaneEdit::ReplaceTexture { index, texture } => {
                let slot = tree.main_bntx.as_mut()?.textures.get_mut(index)?;
                let before = std::mem::replace(slot, (*texture).clone());
                tree.texture_changes.push(texture.name.clone());
                tree.bntx_edited = true;

                Some(AppliedCommand::ReplaceBntxTexture {
                    index,
                    before: Some(Box::new(before)),
                    after: Some(texture),
                })
            }

            PaneEdit::OverrideSection {
                owner,
                pane,
                toggle,
            } => override_section(ctx, owner, &pane, toggle),

            PaneEdit::AddMaterial => {
                let list = tree.material_list.get_or_insert_default();
                let index = pane_factory::add_material(list, "Material");

                Some(AppliedCommand::InsertedMaterial { index })
            }

            PaneEdit::DuplicateMaterial { index } => {
                let list = tree.material_list.as_ref()?;
                let mut copy = list.materials.get(index as usize)?.clone();
                let base = copy.material_name.trim_end_matches('\0').to_string();
                copy.material_name = pane_factory::unique_material_name(list, &base);

                tree.insert_material(index + 1, copy);
                Some(AppliedCommand::InsertedMaterial { index: index + 1 })
            }

            PaneEdit::DeleteMaterial { index } => {
                if tree
                    .material_users
                    .get(&index)
                    .is_some_and(|u| !u.is_empty())
                {
                    return None;
                }

                let material = tree.remove_material(index)?;

                Some(AppliedCommand::RemovedMaterial {
                    index,
                    material: Box::new(material),
                })
            }

            PaneEdit::RenameMaterial { index, name } => {
                if tree.material_name_error(&name, Some(index)).is_some() {
                    return None;
                }

                let material = tree
                    .material_list
                    .as_mut()?
                    .materials
                    .get_mut(index as usize)?;

                let before = material.clone();

                let old = before.material_name.trim_end_matches('\0').to_string();
                if old == name {
                    return None;
                }

                material.material_name = name.clone();
                let after = material.clone();
                tree.mark_material_dirty(index);

                let mut commands = vec![AppliedCommand::ReplaceMaterial {
                    index,
                    before: Box::new(before),
                    after: Box::new(after),
                }];

                commands.extend(edit_anims(ctx.timeline, ctx.tree, |info| {
                    let mut changed = false;

                    for content in &mut info.contents {
                        if content.anim_type == AnimType::Material
                            && content.name.trim_end_matches('\0') == old
                        {
                            content.name = name.clone();
                            changed = true;
                        }
                    }

                    changed
                }));

                Some(AppliedCommand::Batch(commands))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverrideToggle {
    Text,
    Position,
    Shadow,
    Colours,
    Textures,
}

impl OverrideToggle {
    pub fn get(self, prop: &nnbfl::bflyt::pane::PartsProperty) -> bool {
        match self {
            OverrideToggle::Text => prop.usage_flag.use_text,
            OverrideToggle::Position => prop.usage_flag.use_position,
            OverrideToggle::Shadow => prop.usage_flag.use_shadow,
            OverrideToggle::Colours => prop.material_usage_flag.use_color_blend,
            OverrideToggle::Textures => prop.material_usage_flag.use_texture,
        }
    }

    pub fn set(self, prop: &mut nnbfl::bflyt::pane::PartsProperty, on: bool) {
        match self {
            OverrideToggle::Text => prop.usage_flag.use_text = on,
            OverrideToggle::Position => prop.usage_flag.use_position = on,
            OverrideToggle::Shadow => prop.usage_flag.use_shadow = on,
            OverrideToggle::Colours => prop.material_usage_flag.use_color_blend = on,
            OverrideToggle::Textures => prop.material_usage_flag.use_texture = on,
        }
    }
}

fn override_section(
    ctx: &mut EditCtx,
    owner: PaneId,
    pane: &str,
    toggle: OverrideToggle,
) -> Option<AppliedCommand> {
    let tree = &mut *ctx.tree;
    let owner_node = tree.find_by_idx(owner)?;

    if owner_node.parts_source.is_some() {
        return None;
    }

    let before = PaneSnapshot::of(owner_node);
    let content = crate::pane_tree::find_content(&owner_node.parts_children, pane)?;
    let mut section = content.section.clone();
    let list = content.materials.clone();

    let copies: Vec<Material> = crate::pane_tree::section_material_indices(&section)
        .iter()
        .filter_map(|&i| list.as_ref()?.materials.get(i as usize).cloned())
        .collect();

    let mut commands = Vec::new();
    let mut indices = Vec::new();

    for mut material in copies {
        let layout = tree.material_list.get_or_insert_default();
        let original = material.material_name.trim_end_matches('\0');

        let wanted = if original == pane {
            pane.to_string()
        } else {
            format!("{pane}_{original}")
        };

        let wanted: String = wanted.chars().take(MATERIAL_NAME_LEN).collect();
        material.material_name = pane_factory::unique_material_name(layout, &wanted);
        let index = layout.materials.len() as u16;

        tree.insert_material(index, material);
        commands.push(AppliedCommand::InsertedMaterial { index });
        indices.push(index);
    }

    if !indices.is_empty() {
        crate::pane_tree::set_section_materials(&mut section, &indices);
    }

    let node = tree.find_by_idx_mut(owner)?;
    let BflytSection::PartsPane(parts) = &mut node.section else {
        return None;
    };

    let at = match parts
        .properties
        .iter()
        .position(|p| p.property_name.trim_end_matches('\0') == pane)
    {
        Some(at) => at,
        None => {
            parts.properties.push(nnbfl::bflyt::pane::PartsProperty {
                property_name: pane.to_string(),
                ..Default::default()
            });
            parts.properties.len() - 1
        }
    };

    let prop = &mut parts.properties[at];
    prop.o_section = Some(section);
    toggle.set(prop, true);

    let ids = tree.parts_content_ids(owner);
    tree.rebuild_parts(owner, Some(ids), ctx.glyphs);
    tree.rebuild_maps();
    let node = tree.find_by_idx_mut(owner)?;
    node.mark_transform_dirty();
    let after = PaneSnapshot::of(node);

    commands.push(AppliedCommand::ReplacePane {
        pane_idx: owner,
        before: Box::new(before),
        after: Box::new(after),
    });

    Some(AppliedCommand::Batch(commands))
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
    parts: Vec<(PaneId, Vec<PaneId>)>,
}

impl PendingRemoval {
    fn new(parent_idx: Option<PaneId>, position: usize, mut node: PaneNode) -> Self {
        let parts = take_parts_content(&mut node);
        strip_visuals(&mut node);

        Self {
            parent_idx,
            position,
            node,
            parts,
        }
    }
}

fn strip_visuals(node: &mut PaneNode) {
    if node.parts_source.is_none() || node.is_parts_overridden {
        node.textured_quad = None;
        node.window_quads = Vec::new();
        node.glyph_quads = Vec::new();
    }

    for child in node
        .children
        .iter_mut()
        .chain(node.parts_children.iter_mut())
    {
        strip_visuals(child);
    }
}

struct ByteCounter(usize);

impl std::io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn serialized_size<T: serde::Serialize + ?Sized>(value: &T) -> usize {
    let mut counter = ByteCounter(0);
    let _ = serde_json::to_writer(&mut counter, value);
    counter.0
}

fn node_size(node: &PaneNode) -> usize {
    let quads =
        node.textured_quad.is_some() as usize + node.window_quads.len() + node.glyph_quads.len();

    // being conservative by multiplying by 2 cuz actual memory seemed to outweigh the serialized calc
    2 * size_of::<PaneNode>()
        + serialized_size(&node.section)
        + node.label.len()
        + node.user_data.as_ref().map_or(0, serialized_size)
        + quads * size_of::<TexturedQuad>()
        + node
            .children
            .iter()
            .chain(&node.parts_children)
            .map(node_size)
            .sum::<usize>()
}

fn snapshot_size(snapshot: &PaneSnapshot) -> usize {
    serialized_size(&snapshot.section)
        + snapshot.label.len()
        + snapshot.user_data.as_ref().map_or(0, serialized_size)
}

fn meta_size(meta: &LayoutMeta) -> usize {
    meta.user_data.as_ref().map_or(0, serialized_size)
        + serialized_size(&meta.group)
        + meta.control_source.as_ref().map_or(0, serialized_size)
        + meta.material_list.as_ref().map_or(0, serialized_size)
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
    ReplaceMaterial {
        index: u16,
        before: Box<Material>,
        after: Box<Material>,
    },
    ReplaceTextureList {
        before: Box<Option<TextureList>>,
        after: Box<Option<TextureList>>,
    },
    ReplaceBntxTexture {
        index: usize,
        before: Option<Box<Texture>>,
        after: Option<Box<Texture>>,
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
                Some(AppliedCommand::Removed(Box::new(PendingRemoval::new(
                    parent_idx, position, node,
                ))))
            }

            AppliedCommand::Removed(removal) => {
                let PendingRemoval {
                    parent_idx,
                    position,
                    node,
                    parts,
                } = *removal;

                let pane_idx = tree.insert_node_at(parent_idx, position, node);

                let restored: Vec<PaneId> = tree
                    .find_by_idx(pane_idx)
                    .map(|node| node.iter().map(|n| n.pane_idx).collect())
                    .unwrap_or_default();

                for pane in restored {
                    if let Some(node) = tree.find_by_idx_mut(pane) {
                        node.dirty = DirtyFlags::all();
                    }

                    tree.mark_pane_dirty(pane);
                }

                for (pane, ids) in parts {
                    tree.rebuild_parts(pane, Some(ids), ctx.glyphs);
                }

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
                let materials_before = node.material_indices();

                node.section = before.section.clone();
                node.label = before.label.clone();
                node.user_data = before.user_data.clone();
                node.dirty = DirtyFlags::all();
                tree.mark_pane_dirty(pane_idx);

                let rematerialed = tree
                    .find_by_idx(pane_idx)
                    .is_some_and(|n| n.material_indices() != materials_before);

                if renamed || rematerialed {
                    tree.rebuild_maps();
                }

                if matches!(before.section, BflytSection::PartsPane(_)) {
                    let ids = tree.parts_content_ids(pane_idx);
                    tree.rebuild_parts(pane_idx, Some(ids), ctx.glyphs);

                    if let Some(node) = tree.find_by_idx_mut(pane_idx) {
                        node.mark_transform_dirty();
                    }
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

            AppliedCommand::ReplaceMaterial {
                index,
                before,
                after,
            } => {
                let material = tree
                    .material_list
                    .as_mut()?
                    .materials
                    .get_mut(index as usize)?;

                *material = (*before).clone();
                tree.mark_material_dirty(index);

                Some(AppliedCommand::ReplaceMaterial {
                    index,
                    before: after,
                    after: before,
                })
            }

            AppliedCommand::ReplaceBntxTexture {
                index,
                before,
                after,
            } => {
                let textures = &mut tree.main_bntx.as_mut()?.textures;

                match (&before, &after) {
                    (Some(texture), Some(_)) => *textures.get_mut(index)? = (**texture).clone(),
                    (Some(texture), None) => {
                        textures.insert(index.min(textures.len()), (**texture).clone())
                    }
                    (None, Some(_)) => {
                        if index < textures.len() {
                            textures.remove(index);
                        }
                    }
                    (None, None) => {}
                }

                tree.texture_changes
                    .extend(before.iter().chain(&after).map(|t| t.name.clone()));
                tree.bntx_edited = true;

                Some(AppliedCommand::ReplaceBntxTexture {
                    index,
                    before: after,
                    after: before,
                })
            }

            AppliedCommand::ReplaceTextureList { before, after } => {
                tree.texture_list = (*before).clone();
                Some(AppliedCommand::ReplaceTextureList {
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

    pub fn approx_size(&self) -> usize {
        size_of::<Self>()
            + match self {
                AppliedCommand::Inserted(_)
                | AppliedCommand::Transformed { .. }
                | AppliedCommand::Moved { .. }
                | AppliedCommand::InsertedMaterial { .. } => 0,
                AppliedCommand::Removed(removal) => {
                    node_size(&removal.node)
                        + removal
                            .parts
                            .iter()
                            .map(|(_, ids)| (ids.len() + 1) * size_of::<PaneId>())
                            .sum::<usize>()
                }
                AppliedCommand::ReplacePane { before, after, .. } => {
                    snapshot_size(before) + snapshot_size(after)
                }
                AppliedCommand::ReplaceLayoutMeta { before, after } => {
                    meta_size(before) + meta_size(after)
                }
                AppliedCommand::RemovedMaterial { material, .. } => serialized_size(material),
                AppliedCommand::ReplaceAnimInfo { before, after, .. } => {
                    serialized_size(before) + serialized_size(after)
                }
                AppliedCommand::ReplaceMaterial { before, after, .. } => {
                    serialized_size(before) + serialized_size(after)
                }
                AppliedCommand::ReplaceTextureList { before, after } => {
                    serialized_size(before) + serialized_size(after)
                }
                AppliedCommand::ReplaceBntxTexture { before, after, .. } => before
                    .iter()
                    .chain(after)
                    .map(|t| size_of::<Texture>() + t.image_data.len() + t.mip_offsets.len() * 8)
                    .sum(),
                AppliedCommand::Batch(commands) => {
                    commands.iter().map(AppliedCommand::approx_size).sum()
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
            | AppliedCommand::ReplaceAnimInfo { .. }
            | AppliedCommand::ReplaceMaterial { .. }
            | AppliedCommand::ReplaceTextureList { .. }
            | AppliedCommand::ReplaceBntxTexture { .. } => Vec::new(),
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

pub struct MaterialSession {
    pub index: u16,
    pub material: Material,
    pub textures: Option<TextureList>,
}

pub struct PaneSession {
    pub pane: PaneId,
    pub snapshot: PaneSnapshot,
}

struct Step {
    label: String,
    command: AppliedCommand,
    size: usize,
}

impl Step {
    fn new(label: String, command: AppliedCommand) -> Self {
        let estimate = size_of::<Self>() + label.len() + command.approx_size();
        let size = estimate + estimate / 4;

        Self {
            label,
            command,
            size,
        }
    }
}

pub struct EditHistory {
    undo_stack: VecDeque<Step>,
    redo_stack: Vec<Step>,
    limit: usize,
    budget: usize,
    size: usize,
    dropped: bool,
}

impl EditHistory {
    pub fn new(limit: usize, budget: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            limit,
            budget,
            size: 0,
            dropped: false,
        }
    }

    pub fn size(&self) -> usize {
        self.size
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
        let Some(step) = self.undo_stack.pop_back() else {
            return Vec::new();
        };

        self.size -= step.size;
        let Some(inverse) = step.command.invert(ctx) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        let step = Step::new(step.label, inverse);
        self.size += step.size;
        self.redo_stack.push(step);

        panes
    }

    pub fn redo(&mut self, ctx: &mut EditCtx) -> Vec<PaneId> {
        let Some(step) = self.redo_stack.pop() else {
            return Vec::new();
        };

        self.size -= step.size;
        let Some(inverse) = step.command.invert(ctx) else {
            return Vec::new();
        };

        let panes = inverse.resulting_panes();
        self.push_undo(Step::new(step.label, inverse));

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
        self.size += step.size;
        self.undo_stack.push_back(step);

        while self.undo_stack.len() > self.limit
            || (self.size > self.budget && self.undo_stack.len() > 1)
        {
            let Some(oldest) = self.undo_stack.pop_front() else {
                break;
            };

            self.size -= oldest.size;
            self.dropped = true;
        }
    }

    pub fn record(&mut self, label: String, command: AppliedCommand) {
        for step in self.redo_stack.drain(..) {
            self.size -= step.size;
        }

        self.push_undo(Step::new(label, command));
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

    let material = |index: &u16| {
        tree.material_list
            .as_ref()
            .and_then(|list| list.materials.get(*index as usize))
            .map(|m| m.material_name.trim_end_matches('\0').to_string())
            .unwrap_or_default()
    };

    let verb = |edit: &PaneEdit| match edit {
        PaneEdit::Delete { .. } | PaneEdit::DeleteMaterial { .. } => "Delete",
        PaneEdit::Duplicate { .. } | PaneEdit::DuplicateMaterial { .. } => "Duplicate",
        PaneEdit::Add { .. } | PaneEdit::AddMaterial => "Add",
        PaneEdit::OverrideSection { .. } => "Override",
        PaneEdit::ImportTexture { .. } => "Import",
        PaneEdit::EditAnim { .. } => "Edit",
        PaneEdit::ReplaceTexture { .. } => "Replace",
        PaneEdit::Move { .. } => "Reorder",
        PaneEdit::Rename { .. } | PaneEdit::RenameMaterial { .. } => "Rename",
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
            PaneEdit::AddMaterial => "Add material".into(),
            PaneEdit::OverrideSection { owner, pane, .. } => {
                format!("Override {pane} in {}", name(owner))
            }
            PaneEdit::DuplicateMaterial { index } => {
                format!("Duplicate material {}", material(index))
            }
            PaneEdit::DeleteMaterial { index } => format!("Delete material {}", material(index)),
            PaneEdit::ImportTexture { texture } => format!("Import texture {}", texture.name),
            PaneEdit::EditAnim { label, .. } => label.clone(),
            PaneEdit::ReplaceTexture { texture, .. } => {
                format!("Replace texture {}", texture.name)
            }
            PaneEdit::RenameMaterial { index, name: new } => {
                format!("Rename material {} to {new}", material(index))
            }
        },
        [first, rest @ ..] if rest.iter().all(|e| verb(e) == verb(first)) => {
            format!("{} {} panes", verb(first), edits.len())
        }
        _ => format!("{} edits", edits.len()),
    }
}
