use std::cmp;

/// Mutably borrow two *seperate* elements from the given slice.
/// Panics when the indexes are equal or out of bounds.
pub fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);

    let split_at_index = cmp::max(first_index, second_index);

    let (first_slice, second_slice) = items.split_at_mut(split_at_index);

    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

pub struct Transition {
    pub level: u32,
    pub value: u32,
}

pub fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| level >= transition.level)
        .map_or(0, |transition| transition.value)
}
