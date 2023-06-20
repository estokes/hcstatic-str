# Hash Consed Static Short String Storage

This crate is for storing static short strings (up to 256 bytes) in as
compact a way as possible. Instead of each string getting it's own
allocation, and associated padding, header, etc, they are stored
packed into 1 MiB allocations. The length is stored in the allocation,
making the stack size of the Str type 1 word instead of the usual 2
for &str. Because the length is limited to 256 bytes only one extra
byte is used in the heap allocation for the length.
