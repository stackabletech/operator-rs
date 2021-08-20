use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// `compute_hash` returns a hash value calculated from object that is passed in
/// (which needs to implement [`Hash`]) as well as an optional `collision_count` to
/// avoid hash collisions.
///
/// The resulting hash is intended to be used for naming objects uniquely and should not (due to the
/// collision count) used in HashMaps or similar structures.
///
/// This differs from the Kubernetes hashing algorithms in that it creates 64-bit (8 byte) hashes
/// and Kubernetes creates 32-bit (4 byte) hashes.
/// TODO: This could be changed by using an external crate, Rust std library only contains 64-bit hashes
///  https://github.com/stackabletech/operator-rs/issues/125
///
/// # Example
///
/// ```
/// use stackable_operator::controller_utils;
///
/// let hash = controller_utils::compute_hash(123, None);
/// let hash2 = controller_utils::compute_hash(123, Some(1));
///
/// assert_ne!(hash, hash2);
///
/// ```
pub fn compute_hash<T>(resource: T, collision_count: Option<u32>) -> String
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    resource.hash(&mut hasher);

    if let Some(collision_count) = collision_count {
        collision_count.hash(&mut hasher);
    }

    let hash = hasher.finish();

    format!("{:x}", hash)
}
