/// Cache padding that enforces 64 byte alignment.
#[repr(align(64))]
pub struct CachePadded<T>(pub T);
