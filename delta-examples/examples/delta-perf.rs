//! This example demonstrates computing delta indices and finding optimal deltas for a set of files.
//! The example data contains 1457 revisions of the file `diff.c` from <https://github.com/git/git/blob/master/diff.c>
use git_pack_delta::{DeltaIndex, hash};
use rayon::prelude::*;
use std::time::Instant;

use apple_dust::track_alloc;

#[global_allocator]
static ALLOCATOR: track_alloc::AllocTracker<std::alloc::System> =
    track_alloc::AllocTracker::system();

fn load_data() -> Vec<Vec<u8>> {
    use std::io::{Cursor, Read};
    use tar::Archive;
    const SAMPLE_DATA: &[u8] = include_bytes!("diff.tar.zst");
    let cursor = Cursor::new(SAMPLE_DATA);
    let decompressed = zstd::decode_all(cursor).unwrap();
    let mut a = Archive::new(Cursor::new(&decompressed));
    let mut files = vec![];
    for entry in a.entries().unwrap() {
        let mut entry = entry.unwrap();
        let mut buf = Vec::with_capacity(entry.header().size().unwrap() as usize);
        entry.read_to_end(&mut buf).unwrap();
        files.push(buf);
    }
    files
}

type HashAlgorithm = hash::GitRabin;

struct FileInfo {
    data: Vec<u8>,
    index: Option<DeltaIndex<HashAlgorithm>>,
    delta: Option<Vec<u8>>,
    delta_base_idx: Option<usize>,
}

fn w_assert_eq<T: core::fmt::Debug + PartialEq>(actual: T, expected: T) {
    if actual != expected {
        println!("! WARN: expected {:?}, got {:?}", expected, actual);
    }
}

const KB: usize = 1024;
const MB: usize = KB * 1024;

fn check_alloc() {
    let alloc = track_alloc::get_total_bytes();
    if alloc < MB as u64 {
        println!("allocated {:.2} KiB", alloc as f64 / KB as f64);
    } else {
        println!("allocated {:.2} MiB", alloc as f64 / MB as f64);
    }
}

#[expect(clippy::too_many_lines)] // ;)
fn main() {
    let data = load_data();
    // files are already sorted oldest to newest
    println!("loaded {} files", data.len());
    w_assert_eq(data.len(), 1456);

    let original_size = data.iter().map(|info| info.len()).sum::<usize>();

    println!("total original size: {} KiB", original_size / KB);

    track_alloc::reset();
    let mut start = Instant::now();

    let mut infos: Vec<_> = data
        .into_par_iter()
        .map(|d| {
            let index = git_pack_delta::create_delta_index_custom(&d);
            FileInfo {
                data: d,
                index,
                delta: None,
                delta_base_idx: None,
            }
        })
        .collect();

    let duration = start.elapsed();
    println!("computed all delta indices in {:?}", duration);
    check_alloc();
    let success = infos.iter().filter(|info| info.index.is_some()).count();
    w_assert_eq(success, infos.len()); // all files should have a valid delta index
    let percent = (success as f64 / infos.len() as f64) * 100.0;
    println!(
        "successfully created delta indices for {} ({:.2}%)",
        success, percent
    );

    const WINDOW_SIZE: usize = 10;

    track_alloc::reset();
    start = Instant::now();

    for i in 0..(infos.len() - 1) {
        // try to encode older files as deltas against newer ones
        let target = &infos[i];
        let mut best_size = target.data.len() - 1;
        let min = i.saturating_sub(WINDOW_SIZE);
        let max = (i + WINDOW_SIZE + 1).min(infos.len());
        let window = min..max;
        for candidate_idx in window {
            if candidate_idx == i {
                continue;
            }
            let candidate = &infos[candidate_idx];
            let Some(index) = &candidate.index else {
                continue;
            };
            let target = &infos[i];
            let new_len =
                git_pack_delta::get_delta_len(&candidate.data, index, &target.data, best_size);
            let Some(new_len) = new_len else { continue };
            if new_len >= best_size {
                continue;
            }
            best_size = new_len;
            let target = &mut infos[i];
            target.delta_base_idx = Some(candidate_idx);
        }
        let target = &infos[i];
        let Some(idx) = target.delta_base_idx else {
            continue;
        };
        // found the best delta, now encode it.
        let base = &infos[idx];
        let index = base.index.as_ref().unwrap();
        let delta =
            git_pack_delta::write_delta_to_vec(&base.data, index, &target.data, best_size).unwrap();
        let target = &mut infos[i];
        target.delta = Some(delta);
    }

    let duration = start.elapsed();
    println!("computed deltas in {:?}", duration);
    check_alloc();
    let count_delta = infos.iter().filter(|info| info.delta.is_some()).count();
    w_assert_eq(count_delta, infos.len() - 1); // only one of the files is expected to fail delta creation.
    w_assert_eq(&infos.last().unwrap().delta, &None); // the last (and newest) file is expected to have no delta.
    let percent_delta = (count_delta as f64 / infos.len() as f64) * 100.0;
    println!("found deltas for {} ({:.2}%)", count_delta, percent_delta);

    let delta_stats = infos
        .iter()
        .filter_map(|info| info.delta.as_ref())
        .map(|delta| delta.len())
        .collect::<Vec<_>>();
    let max_delta = *delta_stats.iter().max().unwrap();
    println!("largest delta size: {} bytes", max_delta);
    w_assert_eq(max_delta, 7515);
    let min_delta = *delta_stats.iter().min().unwrap();
    println!("smallest delta size: {} bytes", min_delta);
    w_assert_eq(min_delta, 10);
    let avg_delta = delta_stats.iter().sum::<usize>() as f64 / delta_stats.len() as f64;
    println!("average delta size: {:.2} bytes", avg_delta);
    w_assert_eq(avg_delta, 140.69484536082473);

    let compressed_size = infos
        .iter()
        .map(|info| {
            if let Some(delta) = &info.delta {
                delta.len()
            } else {
                info.data.len()
            }
        })
        .sum::<usize>();
    println!(
        "total size after delta compression: {} KiB ({:.3}%)",
        compressed_size / KB,
        (compressed_size as f64 / original_size as f64) * 100.0
    );
    w_assert_eq(compressed_size, 427069);

    // now decode them again

    track_alloc::reset();
    start = Instant::now();

    for info in &infos {
        let Some(delta) = &info.delta else { continue };
        let base_idx = info.delta_base_idx.unwrap();
        let base = &infos[base_idx].data;
        let decoded = git_pack_delta::patch_delta(base, delta).unwrap();
        assert_eq!(decoded, info.data);
    }

    let duration = start.elapsed();
    println!("decoded deltas in {:?}", duration);
    check_alloc();
}

// The output should be like this:

/*
loaded 1456 files
total original size: 181600 KiB
computed all delta indices in 112.8511ms
allocated 261.89 MiB
successfully created delta indices for 1456 (100.00%)
computed deltas in 529.4659ms
allocated 199.91 KiB
found deltas for 1455 (99.93%)
largest delta size: 7515 bytes
smallest delta size: 10 bytes
average delta size: 140.69 bytes
total size after delta compression: 417 KiB (0.230%)
decoded deltas in 15.2978ms
allocated 177.13 MiB
*/
