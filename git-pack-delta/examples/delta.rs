use git_pack_delta::hash::GitRabin;
use git_pack_delta::{create_delta_index_custom, patch_delta, write_delta_to_vec};
use std::{env, fs};

fn main() -> std::io::Result<()> {
    let arg1 = env::args().nth(1).expect("Missing first argument");
    let arg2 = env::args().nth(2).expect("Missing second argument");
    let arg3 = env::args().nth(3).expect("Missing third argument");

    let file1 = fs::read(&arg1)?;
    let file2 = fs::read(&arg2)?;

    let index = create_delta_index_custom::<GitRabin>(&file1).unwrap();

    let delta = write_delta_to_vec(&file1, &index, &file2, file2.len()).unwrap();
    fs::write(&arg3, &delta)?;

    let restored = patch_delta(&file1, &delta)?;
    assert_eq!(restored, file2);

    Ok(())
}
