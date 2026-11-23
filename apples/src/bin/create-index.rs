use apple_dust::*;
use apples::*;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

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
