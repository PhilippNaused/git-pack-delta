use apple_dust::*;
use apples::*;
use git_pack_delta::hash;

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let target = create_modified(base, DEFAULT_DIFFERENCE);

    let index1 = git_pack_delta::create_delta_index_custom::<hash::GitRabin>(base).unwrap();
    let index2 = git_pack_delta::create_delta_index_custom::<hash::CyclicPoly32>(base).unwrap();
    let index3 = git_pack_delta::create_delta_index_custom::<hash::CyclicPoly64>(base).unwrap();

    BenchmarkBuilder::default()
        .add_function(
            (base, &target, &index1),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "GitRabin",
        )
        .add_function(
            (base, &target, &index2),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "CyclicPoly32",
        )
        .add_function(
            (base, &target, &index3),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "CyclicPoly64",
        )
        .add_function(
            (base, &target, &index1),
            |(src, target, index)| {
                git_pack_delta::get_delta_len(src, index, target, usize::MAX).unwrap()
            },
            "GitRabin_len",
        )
        .run()
}
