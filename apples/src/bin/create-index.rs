use apple_dust::*;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

const SAMPLE_DATA: &[u8] = include_bytes!("../../../git-pack-delta/tests/data/diff-delta.c");

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;

    BenchmarkBuilder::default()
        .add_function(
            base,
            |base| git_pack_delta::create_delta_index(base),
            "create_delta_index",
        )
        .run()
}
