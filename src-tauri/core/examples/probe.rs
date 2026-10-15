use ghbdtn_core::{dict::Dictionaries, keymap::Lang};

fn main() {
    let t = std::time::Instant::now();
    let d = Dictionaries::load();
    eprintln!("load {:?}", t.elapsed());
    for arg in std::env::args().skip(1).filter(|w| !w.is_empty()) {
        let (prev, w) = match arg.split_once('+') {
            Some((p, w)) => (Some(p), w),
            None => (None, arg.as_str()),
        };
        let l = if w.chars().any(|c| c.is_ascii_alphabetic()) { Lang::En } else { Lang::Ru };
        println!("{arg}: known={} freq={} fix={:?}", d.known(l, w), d.freq(l, w), ghbdtn_core::typo::fix_after(w, prev, l, &d));
    }
}
