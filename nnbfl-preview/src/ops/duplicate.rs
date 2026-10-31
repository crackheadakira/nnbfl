use std::collections::{HashMap, HashSet};

use nnbfl::{
    bflan::anim_info::AnimType,
    bflyt::{file::BflytSection, list::MATERIAL_NAME_LEN, pane::PANE_NAME_LEN},
};

use crate::{
    edit_history::{AppliedCommand, EditCtx, LayoutMeta, edit_anims},
    pane_tree::{Anchor, PaneId, PaneNode, PaneTree, remap_section_materials, take_parts_content},
};

#[derive(Clone, Copy, Debug)]
pub struct DuplicateOptions {
    pub anims: bool,
    pub groups: bool,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            anims: true,
            groups: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenameRule {
    pub find: String,
    pub replace: String,
}

impl RenameRule {
    fn apply(&self, name: &str) -> Option<String> {
        (!self.find.is_empty() && name.contains(&self.find))
            .then(|| name.replace(&self.find, &self.replace))
    }
}

#[derive(Clone, Debug)]
pub struct NameRow {
    pub old: String,
    pub new: String,
}

#[derive(Clone, Debug)]
pub struct MaterialRow {
    pub index: u16,
    pub old: String,
    pub new: String,
}

#[derive(Clone, Debug, Default)]
pub struct DuplicateNames {
    pub panes: Vec<NameRow>,
    pub materials: Vec<MaterialRow>,
    pub shared: Vec<String>,
}

fn trimmed(name: &str) -> &str {
    name.trim_end_matches('\0')
}

fn own_panes(node: &PaneNode, out: &mut Vec<String>) {
    out.push(trimmed(&node.label).to_string());

    for child in &node.children {
        own_panes(child, out);
    }
}

pub fn plan(tree: &PaneTree, source: PaneId, rule: Option<&RenameRule>) -> Option<DuplicateNames> {
    let node = tree.find_by_idx(source)?;

    if node.parts_source.is_some() || tree.parent_map.get(&source).copied().flatten().is_none() {
        return None;
    }

    let mut olds = Vec::new();
    own_panes(node, &mut olds);

    let mut taken = HashSet::new();

    let panes: Vec<NameRow> = olds
        .into_iter()
        .map(|old| {
            let wanted = rule.and_then(|r| r.apply(&old));

            let new = match wanted {
                Some(name)
                    if name.len() <= PANE_NAME_LEN
                        && !tree.label_map.contains_key(&name)
                        && !taken.contains(&name) =>
                {
                    name
                }
                Some(name) => tree.unique_name(&name, &taken),
                None => tree.unique_name(&old, &taken),
            };

            taken.insert(new.clone());
            NameRow { old, new }
        })
        .collect();

    let subtree: HashSet<PaneId> = node.iter().map(|n| n.pane_idx).collect();

    let mut used: Vec<(u16, PaneId)> = node
        .iter()
        .filter(|n| n.parts_source.is_none())
        .flat_map(|n| {
            n.material_indices()
                .into_iter()
                .map(move |i| (i, n.pane_idx))
        })
        .collect();

    used.sort_by_key(|(i, _)| *i);
    used.dedup_by_key(|(i, _)| *i);

    let list = tree.material_list.as_ref();
    let material_name = |index: u16| {
        list.and_then(|l| l.materials.get(index as usize))
            .map(|m| trimmed(&m.material_name).to_string())
    };

    let existing: HashSet<String> = list
        .map(|l| {
            l.materials
                .iter()
                .map(|m| trimmed(&m.material_name).to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut materials = Vec::new();
    let mut shared = Vec::new();
    let mut material_taken = HashSet::new();

    for (index, user) in used {
        let Some(old) = material_name(index) else {
            continue;
        };

        let own = tree
            .material_users
            .get(&index)
            .is_some_and(|users| users.iter().all(|u| subtree.contains(u)));

        if !own {
            shared.push(old);
            continue;
        }

        let user_label = tree
            .find_by_idx(user)
            .map(|n| trimmed(&n.label).to_string());

        let after_pane = user_label.as_deref().and_then(|label| {
            let rest = old.strip_prefix(label)?;
            let row = panes.iter().find(|r| r.old == label)?;

            Some(format!("{}{rest}", row.new))
        });

        let wanted = rule.and_then(|r| r.apply(&old)).or(after_pane);
        let free = |name: &str| !existing.contains(name) && !material_taken.contains(name);
        let new = match wanted {
            Some(name) if name.len() <= MATERIAL_NAME_LEN && free(&name) => name,
            Some(name) => unique_material_name(&name, &free),
            None => unique_material_name(&old, &free),
        };

        material_taken.insert(new.clone());
        materials.push(MaterialRow { index, old, new });
    }

    Some(DuplicateNames {
        panes,
        materials,
        shared,
    })
}

fn unique_material_name(base: &str, free: &impl Fn(&str) -> bool) -> String {
    let stem = match base.rsplit_once('_') {
        Some((stem, digits))
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            stem
        }
        _ => base,
    };

    (0..)
        .map(|n| {
            let suffix = format!("_{n:02}");
            let end = stem.len().min(MATERIAL_NAME_LEN - suffix.len());
            format!("{}{suffix}", &stem[..end])
        })
        .find(|name| free(name))
        .unwrap()
}

fn name_error(name: &str, limit: usize, spaces: bool) -> Option<String> {
    let allowed = |b: u8| b.is_ascii_graphic() || spaces && b == b' ';

    if name.is_empty() {
        Some("The name is empty".into())
    } else if !name.bytes().all(allowed) {
        Some(
            if spaces {
                "Names are ASCII"
            } else {
                "Names are ASCII without spaces"
            }
            .into(),
        )
    } else if name.len() > limit {
        Some(format!("Names are at most {limit} bytes"))
    } else {
        None
    }
}

pub fn errors(
    tree: &PaneTree,
    names: &DuplicateNames,
) -> (Vec<Option<String>>, Vec<Option<String>>) {
    let count =
        |name: &str, all: &mut dyn Iterator<Item = &str>| all.filter(|n| *n == name).count();

    let panes = names
        .panes
        .iter()
        .map(|row| {
            name_error(&row.new, PANE_NAME_LEN, false).or_else(|| {
                if tree.label_map.contains_key(&row.new) {
                    Some(format!("Another pane is named {}", row.new))
                } else if count(&row.new, &mut names.panes.iter().map(|r| r.new.as_str())) > 1 {
                    Some("Two copies would share this name".into())
                } else {
                    None
                }
            })
        })
        .collect();

    let existing: HashSet<&str> = tree
        .material_list
        .as_ref()
        .map(|l| {
            l.materials
                .iter()
                .map(|m| trimmed(&m.material_name))
                .collect()
        })
        .unwrap_or_default();

    let materials = names
        .materials
        .iter()
        .map(|row| {
            name_error(&row.new, MATERIAL_NAME_LEN, true).or_else(|| {
                if existing.contains(row.new.as_str()) {
                    Some(format!("Another material is named {}", row.new))
                } else if count(
                    &row.new,
                    &mut names.materials.iter().map(|r| r.new.as_str()),
                ) > 1
                {
                    Some("Two copies would share this name".into())
                } else {
                    None
                }
            })
        })
        .collect();

    (panes, materials)
}

pub fn control_bindings(tree: &PaneTree, names: &DuplicateNames) -> Vec<String> {
    let Some(control) = &tree.control_source else {
        return Vec::new();
    };

    names
        .panes
        .iter()
        .filter(|row| {
            control
                .data
                .pane_bindings
                .iter()
                .any(|b| trimmed(b) == row.old)
        })
        .map(|row| row.old.clone())
        .collect()
}

pub fn apply(
    ctx: &mut EditCtx,
    source: PaneId,
    names: DuplicateNames,
    options: DuplicateOptions,
) -> Option<AppliedCommand> {
    let tree = &mut *ctx.tree;
    let (pane_errors, material_errors) = errors(tree, &names);

    if pane_errors
        .iter()
        .chain(&material_errors)
        .any(Option::is_some)
    {
        return None;
    }

    let parent = tree.parent_map.get(&source).copied().flatten()?;
    let mut clone = tree.find_by_idx(source)?.clone();
    take_parts_content(&mut clone);
    tree.reindex_clone(&mut clone);

    let pane_names: HashMap<&str, &str> = names
        .panes
        .iter()
        .map(|r| (r.old.as_str(), r.new.as_str()))
        .collect();

    let material_names: HashMap<&str, &str> = names
        .materials
        .iter()
        .map(|r| (r.old.as_str(), r.new.as_str()))
        .collect();

    fn rename(node: &mut PaneNode, names: &HashMap<&str, &str>) {
        if let Some(new) = names.get(trimmed(&node.label)) {
            node.label = new.to_string();
        }

        for child in &mut node.children {
            rename(child, names);
        }
    }

    rename(&mut clone, &pane_names);

    let meta_before = LayoutMeta::of(tree);

    let list = tree.material_list.get_or_insert_default();
    let mut index_map = HashMap::new();

    for row in &names.materials {
        let mut material = list.materials.get(row.index as usize)?.clone();
        material.material_name = row.new.clone();
        list.materials.push(material);

        index_map.insert(row.index, (list.materials.len() - 1) as u16);
    }

    let remap = |i: u16| index_map.get(&i).copied().unwrap_or(i);

    fn remap_all(node: &mut PaneNode, remap: &impl Fn(u16) -> u16) {
        if node.parts_source.is_none() {
            remap_section_materials(&mut node.section, remap);
        }

        for child in node.all_children_mut() {
            remap_all(child, remap);
        }
    }

    remap_all(&mut clone, &remap);

    if options.groups {
        let groups = std::iter::once(&mut tree.group.data).chain(tree.group.children.iter_mut());

        for group in groups {
            let copies: Vec<String> = group
                .child_names
                .iter()
                .filter_map(|n| pane_names.get(trimmed(n)).map(|new| new.to_string()))
                .collect();

            group.child_names.extend(copies);
        }
    }

    let meta_after = LayoutMeta::of(tree);

    let position = tree.anchor_position(Some(parent), Anchor::After(source))?;
    let idx = tree.insert_node_at(Some(parent), position, clone);

    let parts_panes: Vec<PaneId> = tree
        .find_by_idx(idx)?
        .iter()
        .filter(|n| n.parts_source.is_none() && matches!(n.section, BflytSection::PartsPane(_)))
        .map(|n| n.pane_idx)
        .collect();

    for pane in parts_panes {
        tree.rebuild_parts(pane, None, ctx.glyphs);
    }

    let copies: Vec<PaneId> = tree.find_by_idx(idx)?.iter().map(|n| n.pane_idx).collect();

    for pane in copies {
        tree.mark_pane_dirty(pane);
    }

    let mut commands = vec![
        AppliedCommand::ReplaceLayoutMeta {
            before: Box::new(meta_before),
            after: Box::new(meta_after),
        },
        AppliedCommand::Inserted(idx),
    ];

    if options.anims {
        commands.extend(edit_anims(ctx.timeline, ctx.tree, |info| {
            let copies: Vec<_> = info
                .contents
                .iter()
                .filter_map(|content| {
                    let name = trimmed(&content.name);
                    let new = match content.anim_type {
                        AnimType::Pane | AnimType::PaneExt => pane_names.get(name),
                        AnimType::Material => material_names.get(name),
                        AnimType::User | AnimType::StateMachine => {
                            if pane_names.contains_key(name) {
                                log::info!(
                                    "Not copying the {:?} track of {name}",
                                    content.anim_type
                                );
                            }
                            None
                        }
                    }?;

                    let mut copy = content.clone();
                    copy.name = new.to_string();

                    Some(copy)
                })
                .collect();

            let changed = !copies.is_empty();
            info.contents.extend(copies);

            changed
        }));
    }

    Some(AppliedCommand::Batch(commands))
}
