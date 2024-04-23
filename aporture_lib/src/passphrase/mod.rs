use rand::distributions::{Distribution, Uniform};

mod wordlist;
use wordlist::WORDLIST;

pub fn generate(word_count: usize) -> Vec<u8> {
    Uniform::new(0, WORDLIST.len())
        .sample_iter(rand::thread_rng())
        .take(word_count)
        .map(|i| WORDLIST[i])
        .collect::<Vec<&str>>()
        .join("-")
        .into_bytes()
}
