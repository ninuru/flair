use crate::theme::{Rgb, blend};

pub const MONOGRAM_HUES: [Rgb; 6] = [
    Rgb {
        r: 0x01,
        g: 0x98,
        b: 0xce,
    },
    Rgb {
        r: 0xff,
        g: 0x89,
        b: 0x04,
    },
    Rgb {
        r: 0xfb,
        g: 0x64,
        b: 0xb6,
    },
    Rgb {
        r: 0xe5,
        g: 0x66,
        b: 0xf6,
    },
    Rgb {
        r: 0xff,
        g: 0x63,
        b: 0x7e,
    },
    Rgb {
        r: 0x22,
        g: 0xc5,
        b: 0x5e,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monogram {
    pub hue: Rgb,
    pub text: Rgb,
    pub letters: String,
}

pub fn monogram(name: &str) -> Monogram {
    let hue = MONOGRAM_HUES[hue_index(name)];
    Monogram {
        hue,
        text: blend(Rgb::default(), hue, 20),
        letters: letters(name),
    }
}

fn hue_index(name: &str) -> usize {
    (fnv1a_64(name.as_bytes()) % MONOGRAM_HUES.len() as u64) as usize
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn letters(name: &str) -> String {
    let words = words(name);
    let picked: Vec<char> = match words.as_slice() {
        [first, second, ..] => [first, second]
            .iter()
            .filter_map(|word| word.chars().next())
            .collect(),
        [only] => only.chars().take(2).collect(),
        [] => name.chars().take(2).collect(),
    };
    picked.into_iter().flat_map(char::to_uppercase).collect()
}

fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.' | ' ') {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous = None;
            continue;
        }
        if c.is_uppercase() && previous.is_some_and(char::is_lowercase) {
            words.push(std::mem::take(&mut current));
        }
        current.push(c);
        previous = Some(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
#[path = "monogram_tests.rs"]
mod tests;
