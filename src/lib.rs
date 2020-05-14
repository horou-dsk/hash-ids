use std::collections::VecDeque;

const MIN_ALPHABET_LENGTH: usize = 16;
const SEPERATOR_DIV: f32 = 3.5;
const GUARD_DIV: f32 = 12.0;
const DEFAULT_SEPARATORS: &str = "cfhistuCFHISTU";
const DEFAULT_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";

#[derive(Debug)]
pub enum Error {
    AlphabetTooSmall,
    ContainsSpace,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::AlphabetTooSmall => "Alphabet must contain at least 16 unique characters".fmt(f),
            Error::ContainsSpace => "Alphabet may not contain spaces".fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Default)]
pub struct HashIdsBuilder {
    salt: Vec<char>,
    min_length: usize,
}

#[derive(Debug, Default)]
pub struct HashIdsBuilderWithCustomAlphabet {
    inner: HashIdsBuilder,
    alphabet: Vec<char>,
}

impl HashIdsBuilderWithCustomAlphabet {
    pub fn with_salt(mut self, salt: &str) -> Self {
        self.inner = self.inner.with_salt(salt);
        self
    }

    pub fn with_min_length(mut self, min_length: usize) -> Self {
        self.inner = self.inner.with_min_length(min_length);
        self
    }

    pub fn finish(self) -> std::result::Result<HashIds, Error> {
        let Self { inner: HashIdsBuilder { salt, min_length }, mut alphabet } = self;

        let separators =
            DEFAULT_SEPARATORS.chars().filter(|x| alphabet.contains(x)).collect::<Vec<_>>();

        alphabet = alphabet.drain(..).filter(|x| !separators.contains(x)).collect();

        alphabet = alphabet.clone().into_iter().enumerate()
            .filter(|(i, c)| alphabet.iter().position(|a| a == c) == Some(*i))
            .map(|(_, c)| c)
            .collect();

        if alphabet.len() + separators.len() < MIN_ALPHABET_LENGTH {
            return Err(Error::AlphabetTooSmall);
        }

        if alphabet.contains(&' ') {
            return Err(Error::ContainsSpace);
        }

        Ok(HashIds { salt, min_length, alphabet, separators, guards: Vec::new() }.finish())
    }
}

impl HashIdsBuilder {
    pub fn with_salt(mut self, salt: &str) -> Self {
        self.salt = salt.chars().collect();
        self
    }

    pub fn with_min_length(mut self, min_length: usize) -> Self {
        self.min_length = min_length;
        self
    }

    pub fn with_alphabet(self, alphabet: &str) -> HashIdsBuilderWithCustomAlphabet {
        HashIdsBuilderWithCustomAlphabet { inner: self, alphabet: alphabet.chars().collect() }
    }

    pub fn finish(self) -> HashIds {
        let Self { salt, min_length } = self;
        HashIds {
            salt,
            min_length,
            alphabet: DEFAULT_ALPHABET
                .chars()
                .filter(|x| !DEFAULT_SEPARATORS.contains(*x))
                .collect(),
            separators: DEFAULT_SEPARATORS.chars().collect(),
            guards: Vec::new(),
        }
        .finish()
    }
}

#[derive(Debug, Clone)]
pub struct HashIds {
    salt: Vec<char>,
    min_length: usize,
    alphabet: Vec<char>,
    separators: Vec<char>,
    guards: Vec<char>,
}

impl HashIds {
    fn finish(self) -> Self {
        let Self { salt, min_length, mut alphabet, mut separators, .. } = self;

        let mut guards;

        separators = Self::reorder(&separators, &salt);

        let min_separators = Self::index_from_ratio(alphabet.len(), SEPERATOR_DIV);

        if let Some(num_missing_separators) = min_separators.checked_sub(separators.len()) {
            if num_missing_separators > 0 {
                let mut new_alphabet = alphabet.split_off(num_missing_separators);
                std::mem::swap(&mut alphabet, &mut new_alphabet);
                separators.append(&mut new_alphabet);
            }
        }

        alphabet = Self::reorder(&alphabet, &salt);

        let num_guards = Self::index_from_ratio(alphabet.len(), GUARD_DIV);

        if alphabet.len() < 3 {
            guards = separators.split_off(num_guards);
            std::mem::swap(&mut separators, &mut guards);
        } else {
            guards = alphabet.split_off(num_guards);
            std::mem::swap(&mut alphabet, &mut guards);
        }

        Self { salt, min_length, alphabet, separators, guards }
    }

    fn index_from_ratio(dividend: usize, divisor: f32) -> usize {
        (dividend as f32 / divisor).ceil() as _
    }

    fn reorder(string: &[char], salt: &[char]) -> Vec<char> {
        let mut out = string.to_vec();

        if salt.is_empty() {
            return out;
        }

        let mut int_sum = 0;
        let mut index = 0;

        for i in (1..string.len()).rev() {
            let int = u32::from(salt[index]) as usize;
            int_sum += int;
            let j = (int + index + int_sum) % i;
            out.swap(i, j);
            index = (index + 1) % salt.len();
        }

        out
    }

    fn hash(mut number: usize, alphabet: &[char]) -> Vec<char> {
        let mut hashed = VecDeque::new();
        loop {
            hashed.push_front(alphabet[number % alphabet.len()]);
            number /= alphabet.len();
            if number == 0 {
                break;
            }
        }
        hashed.into_iter().collect()
    }

    fn unhash<I: Iterator<Item = char>>(hashed: I, alphabet: &[char]) -> u64 {
        let mut number: u64 = 0;

        for c in hashed {
            let pos = alphabet.iter().position(|y| y == &c).unwrap() as u64;
            number *= alphabet.len() as u64;
            number += pos;
        }

        number as u64
    }

    fn split<I: Iterator<Item = char>>(string: I, splitters: &[char]) -> Vec<Vec<char>> {
        let mut parts = Vec::new();
        let mut buf = Vec::new();
        for c in string {
            if splitters.contains(&c) {
                parts.push(buf);
                buf = Vec::new();
            } else {
                buf.push(c);
            }
        }
        parts.push(buf);
        parts
    }

    pub fn encode(&self, vals: &[u64]) -> String {

        if vals.is_empty() { return String::new(); }

        let mut alphabet = self.alphabet.clone();

        let vals_hash =
            vals.iter().enumerate().fold(0, |acc, (i, x)| acc + ((*x as usize) % (i + 100)));

        let lottery = self.alphabet[vals_hash % self.alphabet.len()];
        let mut encoded = vec![lottery];

        for (i, mut val) in vals.iter().map(|x| *x as usize).enumerate() {
            let alphabet_salt = std::iter::once(lottery)
                .chain(self.salt.iter().copied())
                .chain(alphabet.iter().copied())
                .take(alphabet.len())
                .collect::<Vec<_>>();

            alphabet = Self::reorder(&alphabet, &alphabet_salt);
            let mut last = Self::hash(val as usize, &alphabet);
            val %= (u32::from(last[0]) as usize) + i;
            encoded.append(&mut last);
            encoded.push(self.separators[val % self.separators.len()]);
        }

        let _ = encoded.pop();

        if encoded.len() >= self.min_length as usize {
            encoded.into_iter().collect::<String>()
        } else {
            let mut encoded = encoded.into_iter().collect::<VecDeque<_>>();

            let mut guard_index = (vals_hash + u32::from(encoded[0]) as usize) % self.guards.len();
            encoded.push_front(self.guards[guard_index]);

            if encoded.len() < self.min_length as usize {
                guard_index = (vals_hash + u32::from(encoded[2]) as usize) % self.guards.len();
                encoded.push_back(self.guards[guard_index]);
            }

            let split_at = alphabet.len() / 2;

            while encoded.len() < self.min_length as usize {
                alphabet = Self::reorder(&alphabet, &alphabet);

                for c in alphabet[split_at..].iter().copied().rev() {
                    encoded.push_front(c);
                }
                for c in alphabet[..split_at].iter().copied() {
                    encoded.push_back(c);
                }
                if let Some(excess) = encoded.len().checked_sub(self.min_length as usize) {
                    if excess > 0 {
                        let from_index = excess / 2;
                        return encoded
                            .drain(from_index..from_index + (self.min_length as usize))
                            .collect::<String>();
                    }
                }
            }

            encoded.into_iter().collect::<String>()
        }
    }
   

    pub fn decode(&self, hash_str: &str) -> Vec<u64> {

        if hash_str.is_empty() { return vec![]; }

        let mut alphabet = self.alphabet.clone();

        let mut parts = Self::split(hash_str.chars(), &self.guards);

        let mut hash_str =
            if parts.len() >= 2 && parts.len() <= 3 { parts.remove(1) } else { parts.remove(0) };

        let lottery = hash_str.remove(0);

        let parts = Self::split(hash_str.iter().copied(), &self.separators);

        let mut out = Vec::with_capacity(parts.len());

        for part in parts {
            let alphabet_salt = std::iter::once(lottery)
                .chain(self.salt.iter().copied())
                .chain(alphabet.iter().copied())
                .take(alphabet.len())
                .collect::<Vec<_>>();
            alphabet = Self::reorder(&alphabet, &alphabet_salt);

            out.push(Self::unhash(part.iter().copied(), &alphabet))
        }

        out
    }
}

#[cfg(test)]
mod tests {

    use super::*;

            // let hash_ids = HashIdsBuilder::default()
        //     .with_salt("aaaaaaaaaaaaaaaaa")
        //     // .with_min_length(10000)
        //     // .with_alphabet("😁⛅🎈🌲🏡🏃😂😍🎉👍😍abcdefghijklmnopqrstuvwxyz")
        //     .finish();
        //     // .finish().unwrap();

        // dbg!(&hash_ids);

        // let val = hash_ids.encode(&[23, 42, 45]);

        // dbg!(&val);

        // let val = hash_ids.decode(&val);

        // dbg!(&val);

    #[test]
    fn test_small_alphabet_with_no_repeating_characters() {
        assert!(HashIdsBuilder::default().with_alphabet("abcdefghijklmno").finish().is_err());
    }

    #[test]
    fn test_small_alphabet_with_repeating_characters() {
        assert!(HashIdsBuilder::default().with_alphabet("abcdecfghijklbmnoa").finish().is_err());
    }

    #[test]
    fn test_empty() {
        let hash_ids = HashIdsBuilder::default().finish();
        assert_eq!("", hash_ids.encode(&[]));
        assert_eq!(Vec::<u64>::new(), hash_ids.decode(""))
    }

    #[test]
    fn test_default_salt() {
        let hash_ids = HashIdsBuilder::default().finish();
        assert_eq!("o2fXhV", hash_ids.encode(&[1, 2, 3]));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("o2fXhV"));
    }


    #[test]
    fn test_single_number() {
        let hash_ids = HashIdsBuilder::default().finish();

        assert_eq!("j0gW", hash_ids.encode(&[12345]));
        assert_eq!("jR", hash_ids.encode(&[1]));
        assert_eq!("Lw", hash_ids.encode(&[22]));
        assert_eq!("Z0E", hash_ids.encode(&[333]));
        assert_eq!("w0rR", hash_ids.encode(&[9999]));

        assert_eq!(vec![12345], hash_ids.decode("j0gW"));
        assert_eq!(vec![1], hash_ids.decode("jR"));
        assert_eq!(vec![22], hash_ids.decode("Lw"));
        assert_eq!(vec![333], hash_ids.decode("Z0E"));
        assert_eq!(vec![9999], hash_ids.decode("w0rR"));
    }

    #[test]
    fn test_multiple_numbers() {
        let hash_ids = HashIdsBuilder::default().finish();

        assert_eq!("vJvi7On9cXGtD", hash_ids.encode(&[683, 94108, 123, 5]));
        assert_eq!("o2fXhV", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("xGhmsW", hash_ids.encode(&[2, 4, 6]));
        assert_eq!("3lKfD", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![683, 94108, 123, 5], hash_ids.decode("vJvi7On9cXGtD"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("o2fXhV"));
        assert_eq!(vec![2, 4, 6], hash_ids.decode("xGhmsW"));
        assert_eq!(vec![99, 25], hash_ids.decode("3lKfD"));
    }

    #[test]
    fn test_salt() {
        let hash_ids = HashIdsBuilder::default().with_salt("Arbitrary string").finish();

        assert_eq!("QWyf8yboH7KT2", hash_ids.encode(&[683, 94108, 123, 5]));
        assert_eq!("neHrCa", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("LRCgf2", hash_ids.encode(&[2, 4, 6]));
        assert_eq!("JOMh1", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![683, 94108, 123, 5], hash_ids.decode("QWyf8yboH7KT2"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("neHrCa"));
        assert_eq!(vec![2, 4, 6], hash_ids.decode("LRCgf2"));
        assert_eq!(vec![99, 25], hash_ids.decode("JOMh1"));
    }

    #[test]
    fn test_alphabet() {
        let hash_ids = HashIdsBuilder::default().with_alphabet(r##"!"#%&',-/0123456789:;<=>ABCDEFGHIJKLMNOPQRSTUVWXYZ_`abcdefghijklmnopqrstuvwxyz~"##).finish().unwrap();

        assert_eq!("_nJUNTVU3", hash_ids.encode(&[2839, 12, 32, 5]));
        assert_eq!("7xfYh2", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("Z6R>", hash_ids.encode(&[23832]));
        assert_eq!("AYyIB", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![2839, 12, 32, 5], hash_ids.decode("_nJUNTVU3"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("7xfYh2"));
        assert_eq!(vec![23832], hash_ids.decode("Z6R>"));
        assert_eq!(vec![99, 25], hash_ids.decode("AYyIB"));
    }

    #[test]
    fn test_min_length() {
        let hash_ids = HashIdsBuilder::default().with_min_length(25).finish();

        assert_eq!("pO3K69b86jzc6krI416enr2B5", hash_ids.encode(&[7452, 2967, 21401]));
        assert_eq!("gyOwl4B97bo2fXhVaDR0Znjrq", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("Nz7x3VXyMYerRmWeOBQn6LlRG", hash_ids.encode(&[6097]));
        assert_eq!("k91nqP3RBe3lKfDaLJrvy8XjV", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![7452, 2967, 21401], hash_ids.decode("pO3K69b86jzc6krI416enr2B5"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("gyOwl4B97bo2fXhVaDR0Znjrq"));
        assert_eq!(vec![6097], hash_ids.decode("Nz7x3VXyMYerRmWeOBQn6LlRG"));
        assert_eq!(vec![99, 25], hash_ids.decode("k91nqP3RBe3lKfDaLJrvy8XjV"));
    }

    #[test]
    fn test_all_parameters() {
        let hash_ids = HashIdsBuilder::default().with_salt("arbitrary salt").with_alphabet("abcdefghijklmnopqrstuvwxyz").with_min_length(16).finish().unwrap();

        assert_eq!("wygqxeunkatjgkrw", hash_ids.encode(&[7452, 2967, 21401]));
        assert_eq!("pnovxlaxuriowydb", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("jkbgxljrjxmlaonp", hash_ids.encode(&[60125]));
        assert_eq!("erdjpwrgouoxlvbx", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![7452, 2967, 21401], hash_ids.decode("wygqxeunkatjgkrw"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("pnovxlaxuriowydb"));
        assert_eq!(vec![60125], hash_ids.decode("jkbgxljrjxmlaonp"));
        assert_eq!(vec![99, 25], hash_ids.decode("erdjpwrgouoxlvbx"));
    }

    #[test]
    fn test_alphabet_without_standard_separators() {
        let hash_ids = HashIdsBuilder::default().with_alphabet("abdegjklmnopqrvwxyzABDEGJKLMNOPQRVWXYZ1234567890").finish().unwrap();

        assert_eq!("X50Yg6VPoAO4", hash_ids.encode(&[7452, 2967, 21401]));
        assert_eq!("GAbDdR", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("5NMPD", hash_ids.encode(&[60125]));
        assert_eq!("yGya5", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![7452, 2967, 21401], hash_ids.decode("X50Yg6VPoAO4"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("GAbDdR"));
        assert_eq!(vec![60125], hash_ids.decode("5NMPD"));
        assert_eq!(vec![99, 25], hash_ids.decode("yGya5"));
    }

    #[test]
    fn test_alphabet_with_two_standard_separators() {
        let hash_ids = HashIdsBuilder::default().with_alphabet("abdegjklmnopqrvwxyzABDEGJKLMNOPQRVWXYZ1234567890uC").finish().unwrap();

        assert_eq!("GJNNmKYzbPBw", hash_ids.encode(&[7452, 2967, 21401]));
        assert_eq!("DQCXa4", hash_ids.encode(&[1, 2, 3]));
        assert_eq!("38V1D", hash_ids.encode(&[60125]));
        assert_eq!("373az", hash_ids.encode(&[99, 25]));

        assert_eq!(vec![7452, 2967, 21401], hash_ids.decode("GJNNmKYzbPBw"));
        assert_eq!(vec![1, 2, 3], hash_ids.decode("DQCXa4"));
        assert_eq!(vec![60125], hash_ids.decode("38V1D"));
        assert_eq!(vec![99, 25], hash_ids.decode("373az"));
    }



}
