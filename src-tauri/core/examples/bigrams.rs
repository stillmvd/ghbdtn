use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

const HELD_EVERY: u64 = 20;
const HELD_MAX: usize = 40_000;
const MIN_COUNT: u32 = 3;

fn cyr(c: char) -> bool {
    matches!(c, 'а'..='я' | 'ё')
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, held, max_pairs, inputs @ ..] = args.as_slice() else {
        eprintln!("bigrams <out> <held> <max_pairs> <sentences.txt>...");
        std::process::exit(1);
    };
    let max_pairs: usize = max_pairs.parse().unwrap();
    let vocab: HashMap<String, u32> = include_str!("../data/ru.freq")
        .lines()
        .filter_map(|l| l.split_once(' '))
        .enumerate()
        .map(|(i, (w, _))| (w.replace('ё', "е"), i as u32))
        .collect();
    let words: HashMap<u32, &str> = vocab.iter().map(|(w, &i)| (i, w.as_str())).collect();
    let mut pairs: HashMap<u64, u32> = HashMap::new();
    let mut held_pairs: Vec<(u32, u32)> = Vec::new();
    let mut n = 0u64;
    for path in inputs {
        let file = BufReader::new(std::fs::File::open(path).unwrap());
        for line in file.lines() {
            let Ok(line) = line else { continue };
            let Some((_, text)) = line.split_once('\t') else { continue };
            n += 1;
            let is_held = n.is_multiple_of(HELD_EVERY);
            let mut prev: Option<u32> = None;
            let mut tok = String::new();
            for c in text.chars().chain(std::iter::once('\n')).flat_map(char::to_lowercase) {
                if cyr(c) || (c == '-' && !tok.is_empty()) {
                    tok.push(if c == 'ё' { 'е' } else { c });
                    continue;
                }
                if !tok.is_empty() {
                    let id = vocab.get(tok.trim_end_matches('-')).copied();
                    if let (Some(p), Some(w)) = (prev, id) {
                        if is_held {
                            held_pairs.push((p, w));
                        } else {
                            *pairs.entry((p as u64) << 32 | w as u64).or_insert(0) += 1;
                        }
                    }
                    prev = id;
                    tok.clear();
                }
                if !c.is_whitespace() {
                    prev = None;
                }
            }
        }
    }
    let mut list: Vec<(u64, u32)> = pairs.into_iter().filter(|&(_, c)| c >= MIN_COUNT).collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list.truncate(max_pairs);
    let mut used: Vec<&str> = list.iter().flat_map(|(k, _)| [words[&((k >> 32) as u32)], words[&(*k as u32)]]).collect();
    used.sort_unstable();
    used.dedup();
    let ids: HashMap<&str, u32> = used.iter().enumerate().map(|(i, w)| (*w, i as u32)).collect();
    let mut recs: Vec<(u32, u32, u8)> = list
        .iter()
        .map(|(k, c)| {
            let q = ((*c as f64).ln() * ghbdtn_core::dict::BIGRAM_SCALE).round().min(255.0) as u8;
            (ids[words[&((k >> 32) as u32)]], ids[words[&(*k as u32)]], q)
        })
        .collect();
    recs.sort_unstable();
    std::fs::write(format!("{out}.words"), used.join("\n") + "\n").unwrap();
    let mut bin = Vec::with_capacity(recs.len() * 7);
    for (a, b, q) in recs {
        bin.extend_from_slice(&a.to_le_bytes()[..3]);
        bin.extend_from_slice(&b.to_le_bytes()[..3]);
        bin.push(q);
    }
    std::fs::write(out, bin).unwrap();
    let step = (held_pairs.len() / HELD_MAX).max(1);
    let mut h = std::io::BufWriter::new(std::fs::File::create(held).unwrap());
    for (p, w) in held_pairs.iter().step_by(step).take(HELD_MAX) {
        writeln!(h, "{} {}", words[p], words[w]).unwrap();
    }
    eprintln!("sentences {n}, pairs kept {}, min count {:?}, held {}", list.len(), list.last().map(|l| l.1), held_pairs.len());
}
