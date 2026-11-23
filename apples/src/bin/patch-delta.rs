use apple_dust::*;
use apples::*;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let target = create_modified(base, DEFAULT_DIFFERENCE);

    let delta = git_pack_delta::create_delta(base, &target).unwrap();
    let delta = &delta;

    BenchmarkBuilder::default()
        .add_function(
            (base, delta),
            |(base, delta)| git_pack_delta::patch_delta(base, delta).unwrap(),
            "patch_delta",
        )
        .run()
}
