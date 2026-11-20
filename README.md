# git-pack-delta

This is a pure rust implementation of Git's delta compression algorithm. (Not to be confused with it's diff algorithm!)

```rust
let base = "This is a pure rust implementation of Git's delta compression algorithm.";
let target = base.replace("Git", "git");
let delta = git_pack_delta::create_delta(base.as_bytes(), target.as_bytes()).unwrap();
assert_eq!(delta.len(), 9);
let target2 = git_pack_delta::patch_delta(base.as_bytes(), &delta).unwrap();
assert_eq!(target.as_bytes(), target2);
```

Includes extra features such as faster hash algorithms, calculating delta length without encoding, and SIMD-acceleration.