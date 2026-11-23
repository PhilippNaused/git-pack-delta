use apple_dust::*;
use apples::*;
use git_pack_delta::hash::{self, RollingHash};

fn main() -> Result<(), std::io::Error> {
    let data = &SAMPLE_DATA[..16];

    BenchmarkBuilder::default()
        .add_function(data, |s| hash::GitRabin::value_from_slice(s), "GitRabin")
        .add_function(
            data,
            |s| hash::CyclicPoly32::value_from_slice(s),
            "CyclicPoly32",
        )
        .add_function(
            data,
            |s| hash::CyclicPoly64::value_from_slice(s),
            "CyclicPoly64",
        )
        .run()
}
