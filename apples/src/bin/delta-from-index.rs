use apple_dust::*;
use apples::*;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let target5 = create_modified(base, DEFAULT_DIFFERENCE);
    let target0 = create_modified(base, 0.00);
    let target50 = create_modified(base, 0.50);

    let index = git_pack_delta::create_delta_index(base).unwrap();

    BenchmarkBuilder::default()
        .add_function(
            (base, &target5, &index),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "write_delta_to_05",
        )
        .add_function(
            (base, &target0, &index),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "write_delta_to_00",
        )
        .add_function(
            (base, &target50, &index),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "write_delta_to_50",
        )
        .run()
}
