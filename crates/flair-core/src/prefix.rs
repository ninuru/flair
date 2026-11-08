use std::collections::HashSet;

use crate::{
    command::Command,
    key::{Key, KeyAction, KeyInput, NamedKey},
    keys::{KeyChord, KeysConfig},
    time::Millis,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixResult {
    Forward,
    Consume,
    Command(Command),
    SendPrefix,
}

#[derive(Debug, Default)]
pub struct PrefixMachine {
    pending: Option<(u64, Millis)>,
    generation: u64,
    suppressed: HashSet<(Key, bool)>,
    ime: bool,
}

impl PrefixMachine {
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn deadline(&self) -> Option<(u64, Millis)> {
        self.pending
    }

    pub fn reset(&mut self) {
        self.pending = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_ime(&mut self, composing: bool) {
        self.ime = composing;
        if composing {
            self.reset();
        }
    }

    pub fn expire(&mut self, generation: u64, now: Millis) -> bool {
        if self
            .pending
            .is_some_and(|(current, until)| current == generation && now >= until)
        {
            self.reset();
            true
        } else {
            false
        }
    }

    pub fn handle(&mut self, key: &KeyInput, now: Millis, config: &KeysConfig) -> PrefixResult {
        if let Some((generation, _)) = self.pending {
            self.expire(generation, now);
        }
        let identity = (key.unshifted.map_or(key.key, Key::Char), key.keypad);
        if key.action == KeyAction::Release {
            return if self.suppressed.remove(&identity) {
                PrefixResult::Consume
            } else {
                PrefixResult::Forward
            };
        }
        if self.ime {
            if !key.repeat {
                self.suppressed.insert(identity);
            }
            return PrefixResult::Consume;
        }
        if !key.repeat {
            self.suppressed.remove(&identity);
        } else if self.suppressed.contains(&identity) {
            return PrefixResult::Consume;
        }
        if !config.enabled {
            self.reset();
            return PrefixResult::Forward;
        }
        if matches!(
            key.key,
            Key::Named(
                NamedKey::ShiftLeft
                    | NamedKey::ShiftRight
                    | NamedKey::ControlLeft
                    | NamedKey::ControlRight
                    | NamedKey::AltLeft
                    | NamedKey::AltRight
                    | NamedKey::SuperLeft
                    | NamedKey::SuperRight
            )
        ) {
            return PrefixResult::Forward;
        }
        let prefix = config
            .key
            .parse::<KeyChord>()
            .is_ok_and(|chord| chord.matches(key));
        if self.pending() {
            self.reset();
            if prefix && !key.repeat {
                return PrefixResult::SendPrefix;
            }
            if key.repeat {
                return PrefixResult::Consume;
            }
            self.suppressed.insert(identity);
            return config
                .binding(key, true)
                .map_or(PrefixResult::Consume, PrefixResult::Command);
        }
        if prefix {
            if !key.repeat {
                self.suppressed.insert(identity);
                self.generation = self.generation.wrapping_add(1);
                self.pending = Some((self.generation, now + Millis(config.timeout_ms)));
            }
            return PrefixResult::Consume;
        }
        if let Some(command) = config.binding(key, false) {
            if key.repeat {
                return PrefixResult::Consume;
            }
            self.suppressed.insert(identity);
            return PrefixResult::Command(command);
        }
        PrefixResult::Forward
    }
}

#[cfg(test)]
#[path = "prefix_tests.rs"]
mod tests;
