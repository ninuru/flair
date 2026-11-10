use std::{collections::BTreeMap, path::Path};

use flair_core::{config::PromptConfig, ids::PaneId, isles::PromptInfo};

#[derive(Clone, Copy)]
pub struct Isles<'a> {
    pub info: &'a BTreeMap<PaneId, PromptInfo>,
    pub config: &'a PromptConfig,
    pub clock: Option<(u8, u8)>,
    pub home: Option<&'a Path>,
}
