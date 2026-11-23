use apple_dust::*;
use apples::*;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let target = create_modified(base, DEFAULT_DIFFERENCE);

    let index = git_pack_delta::create_delta_index(base).unwrap();

    BenchmarkBuilder::default()
        .add_function(
            base,
            |base| git_pack_delta::create_delta_index(base),
            "create_delta_index",
        )
        .add_function(
            (base, &target, index),
            |(base, target, index)| {
                git_pack_delta::write_delta_to_vec(base, index, target, usize::MAX).unwrap()
            },
            "write_delta_to_vec",
        )
        .run()
}
