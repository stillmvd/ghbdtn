use ghbdtn_core::{dict::Dictionaries, keymap::Lang};

fn main() {
    let d = Dictionaries::load();
    for w in std::env::args().skip(1) {
        let l = if w.chars().any(|c| c.is_ascii_alphabetic()) { Lang::En } else { Lang::Ru };
        println!("{w}: known={} freq={} fix={:?}", d.known(l, &w), d.freq(l, &w), ghbdtn_core::typo::best_fix(&w, l, &d));
    }
}
