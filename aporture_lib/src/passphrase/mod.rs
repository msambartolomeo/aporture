use rand::distributions::{Distribution, Uniform};

mod wordlist;
use wordlist::WORDLIST;

pub fn generate(word_count: usize) -> String {
    Uniform::new(0, WORDLIST.len())
        .sample_iter(rand::thread_rng())
        .take(word_count)
        .map(|i| WORDLIST[i])
        .collect::<Vec<&str>>()
        .join("-")
}

#[cfg(test)]
mod test {
    use super::*;

    const WORD_COUNT: usize = 4;

    #[test]
    fn test_password_generation_from_wordlist() {
        let pass = generate(WORD_COUNT);

        for w in pass.split('-') {
            assert!(WORDLIST.contains(&w));
        }
    }

    #[test]
    fn test_password_generation_length() {
        let pass = generate(WORD_COUNT);

        assert_eq!(WORD_COUNT, pass.split('-').count());
    }
}
