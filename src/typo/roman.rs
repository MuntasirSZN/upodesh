pub(crate) const INF: u16 = u16::MAX;
const ROMAN_MAX: usize = 40;
const ROW: usize = ROMAN_MAX + 1;

/// Compact byte-trie for Roman spellings. Terminal entries point into a
/// shared word blob holding Bangla strings.
pub(crate) struct RomanTrie {
    pub(crate) start: Vec<u32>,
    pub(crate) cnt: Vec<u32>,
    pub(crate) eid: Vec<u8>,
    pub(crate) ech: Vec<u32>,
    pub(crate) t_start: Vec<u32>,
    pub(crate) t_alloc: Vec<u32>,
    pub(crate) t_off: Vec<u32>,
    pub(crate) t_len: Vec<u32>,
    pub(crate) words: Vec<u8>,
}

/// Trie node under construction: (outgoing edges, terminals).
type Node = (Vec<(u8, u32)>, Vec<(u32, u32)>);
/// One index entry: (Roman form, word offset, word length).
type Pair = (Vec<u8>, u32, u32);

impl RomanTrie {
    pub(crate) fn empty() -> Self {
        Self {
            start: vec![0],
            cnt: vec![0],
            eid: Vec::new(),
            ech: Vec::new(),
            t_start: vec![0],
            t_alloc: vec![0],
            t_off: Vec::new(),
            t_len: Vec::new(),
            words: Vec::new(),
        }
    }

    pub(crate) fn word(&self, off: u32, len: u32) -> &str {
        let off = off as usize;
        let len = len as usize;
        let s = &self.words[off..off + len];
        std::str::from_utf8(s).unwrap_or("")
    }

    /// Parse one v2 trie payload. Layout after `n` header sizes is consumed
    /// by the caller via the returned cursor.
    pub(crate) fn read_from(
        bytes: &[u8],
        cursor: &mut usize,
    ) -> Result<(Self, Vec<u8>), super::BuildError> {
        let read_u32 = |cursor: &mut usize, reason: &'static str| -> Result<u32, super::BuildError> {
            let chunk = bytes
                .get(*cursor..*cursor + 4)
                .ok_or(super::BuildError::InvalidIndex(reason))?;
            *cursor += 4;
            Ok(u32::from_le_bytes(chunk.try_into().expect("4 bytes")))
        };
        let read_vec_u32 = |count: usize, cursor: &mut usize, reason: &'static str| -> Result<Vec<u32>, super::BuildError> {
            let mut out = Vec::with_capacity(count);
            for _ in 0..count {
                out.push(read_u32(cursor, reason)?);
            }
            Ok(out)
        };
        let n = read_u32(cursor, "truncated roman nodes")? as usize;
        let start = read_vec_u32(n, cursor, "truncated roman start")?;
        let cnt = read_vec_u32(n, cursor, "truncated roman cnt")?;
        let total: usize = cnt.iter().map(|c| *c as usize).sum();
        let eid_end = cursor
            .checked_add(total)
            .ok_or(super::BuildError::InvalidIndex("roman eid overflow"))?;
        let eid = bytes
            .get(*cursor..eid_end)
            .ok_or(super::BuildError::InvalidIndex("truncated roman eid"))?
            .to_vec();
        *cursor = eid_end;
        let ech = read_vec_u32(total, cursor, "truncated roman ech")?;
        let t_start = read_vec_u32(n, cursor, "truncated roman t_start")?;
        let t_alloc = read_vec_u32(n, cursor, "truncated roman t_alloc")?;
        let t_words: usize = t_alloc.iter().map(|c| *c as usize).sum();
        let t_off = read_vec_u32(t_words, cursor, "truncated roman t_off")?;
        let t_len = read_vec_u32(t_words, cursor, "truncated roman t_len")?;
        Ok((
            Self {
                start,
                cnt,
                eid,
                ech,
                t_start,
                t_alloc,
                t_off,
                t_len,
                words: Vec::new(),
            },
            Vec::new(),
        ))
    }

    /// Build one trie from (form bytes, word address) pairs.
    pub(crate) fn build(pairs: &mut [Pair], words_blob: Vec<u8>) -> Self {
        let mut nodes: Vec<Node> = vec![(Vec::new(), Vec::new())];
        for (form, off, len) in pairs.iter() {
            let mut node = 0usize;
            for &b in form.iter() {
                let pos = nodes[node].0.iter().position(|(l, _)| *l == b);
                match pos {
                    Some(p) => node = nodes[node].0[p].1 as usize,
                    None => {
                        nodes.push((Vec::new(), Vec::new()));
                        let child = (nodes.len() - 1) as u32;
                        nodes[node].0.push((b, child));
                        node = child as usize;
                    }
                }
            }
            nodes[node].1.push((*off, *len));
        }
        let n = nodes.len();
        let total: usize = nodes.iter().map(|(c, _)| c.len()).sum();
        let mut start = vec![0u32; n];
        let mut cnt = vec![0u32; n];
        let mut eid = Vec::with_capacity(total);
        let mut ech = Vec::with_capacity(total);
        let mut t_start = vec![0u32; n];
        let mut t_alloc = vec![0u32; n];
        let mut t_off = Vec::new();
        let mut t_len = Vec::new();
        let mut offset = 0usize;
        let mut toffset = 0usize;
        for (i, (ch, terms)) in nodes.iter().enumerate() {
            start[i] = offset as u32;
            cnt[i] = ch.len() as u32;
            for &(b, c) in ch {
                eid.push(b);
                ech.push(c);
            }
            offset += ch.len();
            t_start[i] = toffset as u32;
            t_alloc[i] = terms.len() as u32;
            for &(o, l) in terms {
                t_off.push(o);
                t_len.push(l);
            }
            toffset += terms.len();
        }
        Self {
            start,
            cnt,
            eid,
            ech,
            t_start,
            t_alloc,
            t_off,
            t_len,
            words: words_blob,
        }
    }

    pub(crate) fn set_words(&mut self, words: Vec<u8>) {
        self.words = words;
    }
}

#[derive(Clone, Copy)]
struct RFrame {
    node: u32,
    next_child: u32,
    row: [u16; ROW],
    two: [u16; ROW],
    has_two: bool,
    row_min: u16,
    prev: u8,
    has_prev: bool,
    visited: bool,
}

fn sub_cost(q: u16, cand: u8, query: &[u8]) -> u16 {
    let _ = query;
    if q == cand as u16 {
        0
    } else {
        // crude QWERTY neighbor discount for single-letter slips
        neighbor_cost(q as u8, cand)
    }
}

fn neighbor_cost(a: u8, b: u8) -> u16 {
    const GROUPS: &[&[u8]] = &[
        b"qwerty",
        b"asdfgh",
        b"zxcvbn",
        b"1234567890",
    ];
    for group in GROUPS {
        if let (Some(pa), Some(pb)) = (group.iter().position(|c| *c == a), group.iter().position(|c| *c == b))
            && pa.abs_diff(pb) == 1 {
                return 60;
            }
    }
    100
}

fn advance(
    parent: &[u16; ROW],
    two_back: Option<&[u16; ROW]>,
    prev: Option<u8>,
    cand: u8,
    query_len: usize,
    query: &[u8],
    costs: &RomanCosts,
) -> ([u16; ROW], u16) {
    let mut row = [INF; ROW];
    row[0] = parent[0].saturating_add(costs.insertion);
    let mut min = row[0];
    for i in 1..=query_len {
        let delete = parent[i].saturating_add(costs.deletion);
        // Repeated-char rule: a doubled letter in the query (e.g. "banngla"
        // for "bangla") matching the just-consumed trie byte is a much more
        // likely slip than a random stray letter — half-price insertion.
        let insertion = if query[i - 1] == cand {
            costs.insertion / 2
        } else {
            costs.insertion
        };
        let insert = row[i - 1].saturating_add(insertion);
        let sub = parent[i - 1].saturating_add(sub_cost(query[i - 1] as u16, cand, query));
        let mut best = delete.min(insert).min(sub);
        if i >= 2
            && let (Some(pv), Some(two)) = (prev, two_back)
                && query[i - 1] == pv && query[i - 2] == cand {
                    let t = two[i - 2].saturating_add(costs.transposition);
                    if t < best {
                        best = t;
                    }
                }
        row[i] = best;
        if best < min {
            min = best;
        }
    }
    (row, min)
}

pub(crate) struct RomanCosts {
    pub insertion: u16,
    pub deletion: u16,
    pub transposition: u16,
}

/// Bounded weighted search over the Roman trie. Returns deduped
/// (word, cost) pairs whose cost is within `max_cost`.
pub(crate) fn roman_search(
    trie: &RomanTrie,
    roman: &str,
    max_cost: u16,
    costs: &RomanCosts,
) -> Vec<(String, u16)> {
    let query_owned = roman.trim().to_ascii_lowercase();
    let query = query_owned.as_bytes();
    let query_len = query.len().min(ROMAN_MAX);
    if query_len == 0 || trie.cnt[0] == 0 {
        return Vec::new();
    }

    let start = |deletion: u16, len: usize| -> ([u16; ROW], u16) {
        let mut row = [INF; ROW];
        row[0] = 0;
        for (i, cell) in row.iter_mut().enumerate().skip(1).take(len) {
            *cell = (i as u16).saturating_mul(deletion);
        }
        (row, 0)
    };
    let (root, root_min) = start(costs.deletion, query_len);
    let mut stack = vec![RFrame {
        node: 0,
        next_child: 0,
        row: root,
        two: [INF; ROW],
        has_two: false,
        row_min: root_min,
        prev: 0,
        has_prev: false,
        visited: false,
    }];
    let mut out: Vec<(String, u16)> = Vec::new();
    while let Some(frame) = stack.last_mut() {
        if !frame.visited {
            frame.visited = true;
            let live = frame.row_min <= max_cost
                || (frame.has_prev
                    && frame.has_two
                    && (0..query_len).any(|i| {
                        frame.two[i].saturating_add(costs.transposition) <= max_cost
                            && query.get(i + 1) == Some(&frame.prev)
                    }));
            if !live {
                stack.pop();
                continue;
            }
            let depth = frame.node;
            let _ = depth;
            let t_start = trie.t_start[frame.node as usize];
            let t_count = trie.t_alloc[frame.node as usize];
            if t_count > 0 {
                let edit = frame.row[query_len];
                if edit <= max_cost {
                    for k in 0..t_count {
                        let idx = (t_start + k) as usize;
                        let word = trie.word(trie.t_off[idx], trie.t_len[idx]);
                        out.push((word.to_owned(), edit));
                    }
                }
            }
        }
        let node = frame.node;
        let s = trie.start[node as usize] as usize;
        let c = trie.cnt[node as usize] as usize;
        if (frame.next_child as usize) >= c {
            stack.pop();
            continue;
        }
        let edge = s + frame.next_child as usize;
        frame.next_child += 1;
        let cand = trie.eid[edge];
        let child = trie.ech[edge];
        let frame_row = frame.row;
        let frame_two = frame.two;
        let frame_has_two = frame.has_two;
        let frame_prev = frame.prev;
        let frame_has_prev = frame.has_prev;
        let (row, min) = advance(
            &frame_row,
            frame_has_two.then_some(&frame_two),
            frame_has_prev.then_some(frame_prev),
            cand,
            query_len,
            &query[..query_len],
            costs,
        );
        if min > max_cost && !frame_has_two {
            // still allow transposition recovery via two-back
            let mut ok = false;
            if let Some(two) = frame_has_two.then_some(&frame_two) {
                for i in 2..=query_len {
                    if query[i - 1] == frame_prev && two[i - 2].saturating_add(costs.transposition) <= max_cost {
                        ok = true;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
        }
        if min > max_cost {
            // keep only if a transposition could still recover
            let mut ok = false;
            if frame_has_two {
                for i in 2..=query_len {
                    if query[i - 1] == frame_prev
                        && frame_two[i - 2].saturating_add(costs.transposition) <= max_cost
                    {
                        ok = true;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
        }
        stack.push(RFrame {
            node: child,
            next_child: 0,
            row,
            two: frame_row,
            has_two: true,
            row_min: min,
            prev: cand,
            has_prev: true,
            visited: false,
        });
    }
    out.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    out.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    out
}
