use apple_dust::*;
use apples::*;
use git_pack_delta::hash;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;

    BenchmarkBuilder::default()
        .add_function(
            base,
            |base| git_pack_delta::create_delta_index_custom::<hash::GitRabin>(base),
            "GitRabin",
        )
        .add_function(
            base,
            |base| git_pack_delta::create_delta_index_custom::<hash::CyclicPoly32>(base),
            "CyclicPoly32",
        )
        .add_function(
            base,
            |base| git_pack_delta::create_delta_index_custom::<hash::CyclicPoly64>(base),
            "CyclicPoly64",
        )
        .run()
}
