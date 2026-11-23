use apple_dust::*;
use apples::*;
use git_pack_delta::match_bytes::*;

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let mut dex = SAMPLE_DATA.to_vec();
    dex[1000] = 0;
    let dex = dex.as_slice();

    let mut b = BenchmarkBuilder::default().add_function(
        (base, dex),
        |(base, dex)| count_matches_u128(base, dex),
        "count_matches_u128",
    );
    #[cfg(target_feature = "sse2")]
    {
        b = b.add_function(
            (base, dex),
            |(base, dex)| unsafe { count_matches_sse2(base, dex) },
            "count_matches_sse2",
        );
    }
    #[cfg(target_feature = "avx2")]
    {
        b = b.add_function(
            (base, dex),
            |(base, dex)| unsafe { count_matches_avx2(base, dex) },
            "count_matches_avx2",
        );
    }
    #[cfg(target_feature = "avx512bw")]
    {
        b = b.add_function(
            (base, dex),
            |(base, dex)| unsafe { count_matches_avx512bw(base, dex) },
            "count_matches_avx512bw",
        );
    }
    b = b.add_function(
        (base, dex),
        |(base, dex)| count_matches(base, dex),
        "count_matches",
    );
    b.run()
}
